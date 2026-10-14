//! One USB/IP import session. A single event thread owns every native transfer;
//! a blocking socket reader feeds it through a bounded channel.
use anyhow::{Context as _, bail};
use common_utils::Deferred;
use libusb1_sys::{self as usb, constants::*};
use rusb::{TransferType, UsbContext};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::ptr::NonNull;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread;
use std::time::Duration;

use crate::device::UsbDevice;
use crate::protocol::{self, Header, Request, Submit, error_status};

// Bound memory held by the remote peer, independently of device throughput.
const MAX_PENDING_BYTES: usize = 64 * 1024 * 1024;
const MAX_PENDING_TRANSFERS: usize = 128;
const EVENT_INTERVAL: Duration = Duration::from_millis(5);

#[derive(Clone, Copy, PartialEq, Eq)]
enum TransferKind {
    Control,
    Bulk,
    Interrupt,
}

struct Transfer {
    native: NonNull<usb::libusb_transfer>,
    buffer: Vec<u8>,
    completed: Box<Cell<bool>>,
    request: Submit,
    data_offset: usize,
    expected: usize,
    interface: Option<u8>,
    unlink: Option<Header>,
}

// libusb calls this only from handle_events on the owning event thread.
// The boxed marker, native transfer and buffer stay alive until completion.
extern "system" fn completed(native: *mut usb::libusb_transfer) {
    unsafe {
        (*((*native).user_data as *const Cell<bool>)).set(true);
    }
}

impl Transfer {
    fn new(
        device: &UsbDevice,
        mut request: Submit,
        kind: TransferKind,
        interface: Option<u8>,
    ) -> rusb::Result<Self> {
        let native =
            NonNull::new(unsafe { usb::libusb_alloc_transfer(0) }).ok_or(rusb::Error::NoMem)?;
        let data_offset = if kind == TransferKind::Control { 8 } else { 0 };
        let expected = request.buffer.len();
        let mut buffer = std::mem::take(&mut request.buffer);
        if data_offset != 0 {
            buffer.splice(..0, request.setup);
        }
        let mut transfer = Self {
            native,
            buffer,
            completed: Box::new(Cell::new(false)),
            request,
            data_offset,
            expected,
            interface,
            unlink: None,
        };
        let address = transfer.request.header.endpoint
            | if transfer.request.header.input {
                0x80
            } else {
                0
            };
        let marker = (&*transfer.completed as *const Cell<bool>)
            .cast_mut()
            .cast();
        // All pointers refer to allocations owned by `transfer`, not its stack address.
        unsafe {
            match kind {
                TransferKind::Control => usb::libusb_fill_control_transfer(
                    native.as_ptr(),
                    device.handle.as_raw(),
                    transfer.buffer.as_mut_ptr(),
                    completed,
                    marker,
                    0,
                ),
                TransferKind::Bulk => usb::libusb_fill_bulk_transfer(
                    native.as_ptr(),
                    device.handle.as_raw(),
                    address,
                    transfer.buffer.as_mut_ptr(),
                    expected as i32,
                    completed,
                    marker,
                    0,
                ),
                TransferKind::Interrupt => usb::libusb_fill_interrupt_transfer(
                    native.as_ptr(),
                    device.handle.as_raw(),
                    address,
                    transfer.buffer.as_mut_ptr(),
                    expected as i32,
                    completed,
                    marker,
                    0,
                ),
            }
            if transfer.request.flags & 0x40 != 0
                && kind == TransferKind::Bulk
                && !transfer.request.header.input
            {
                (*native.as_ptr()).flags |= LIBUSB_TRANSFER_ADD_ZERO_PACKET;
            }
        }
        Ok(transfer)
    }

    fn start(&mut self) -> rusb::Result<()> {
        native_result(unsafe { usb::libusb_submit_transfer(self.native.as_ptr()) })?;
        Ok(())
    }

    fn cancel(&self) -> rusb::Result<()> {
        native_result(unsafe { usb::libusb_cancel_transfer(self.native.as_ptr()) })
    }

    fn write_completion(&mut self, stream: &mut UnixStream) -> anyhow::Result<bool> {
        // Completion has run, so libusb no longer owns the buffer.
        let native = unsafe { self.native.as_ref() };
        let disconnected = native.status == LIBUSB_TRANSFER_NO_DEVICE;
        let mut status = match native.status {
            LIBUSB_TRANSFER_COMPLETED => 0,
            LIBUSB_TRANSFER_TIMED_OUT => -110,
            LIBUSB_TRANSFER_CANCELLED => -104,
            LIBUSB_TRANSFER_STALL => -32,
            LIBUSB_TRANSFER_NO_DEVICE => -19,
            LIBUSB_TRANSFER_OVERFLOW => -75,
            _ => -5,
        };
        // libusb leaves actual_length undefined on overflow; no payload is usable.
        let actual = if native.status == LIBUSB_TRANSFER_OVERFLOW {
            0
        } else {
            native.actual_length as usize
        };
        if status == 0 && self.request.flags & 1 != 0 && actual < self.expected {
            status = -121;
        }
        if let Some(header) = self.unlink {
            protocol::write_unlink(stream, header, status)?;
        } else {
            protocol::write_submit(
                stream,
                &self.request,
                status,
                &self.buffer[self.data_offset..self.data_offset + actual],
                actual,
            )?;
        }
        Ok(disconnected)
    }
}

impl Drop for Transfer {
    fn drop(&mut self) {
        // Session cleanup drains cancellation callbacks before dropping transfers.
        unsafe {
            usb::libusb_free_transfer(self.native.as_ptr());
        }
    }
}

fn native_result(status: i32) -> rusb::Result<()> {
    match status {
        0 => Ok(()),
        LIBUSB_ERROR_INVALID_PARAM => Err(rusb::Error::InvalidParam),
        LIBUSB_ERROR_ACCESS => Err(rusb::Error::Access),
        LIBUSB_ERROR_NO_DEVICE => Err(rusb::Error::NoDevice),
        LIBUSB_ERROR_NOT_FOUND => Err(rusb::Error::NotFound),
        LIBUSB_ERROR_BUSY => Err(rusb::Error::Busy),
        LIBUSB_ERROR_TIMEOUT => Err(rusb::Error::Timeout),
        LIBUSB_ERROR_OVERFLOW => Err(rusb::Error::Overflow),
        LIBUSB_ERROR_PIPE => Err(rusb::Error::Pipe),
        LIBUSB_ERROR_INTERRUPTED => Err(rusb::Error::Interrupted),
        LIBUSB_ERROR_NO_MEM => Err(rusb::Error::NoMem),
        LIBUSB_ERROR_NOT_SUPPORTED => Err(rusb::Error::NotSupported),
        _ => Err(rusb::Error::Io),
    }
}

pub fn forward(
    mut device: UsbDevice,
    mut stream: UnixStream,
    stopped: &AtomicBool,
) -> anyhow::Result<()> {
    stream
        .set_read_timeout(None)
        .context("Clear USB/IP handshake timeout")?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .context("Set USB/IP reply timeout")?;
    let mut reader = stream.try_clone().context("Clone USB/IP stream")?;
    let device_id = device.id;
    let reader_context = device.handle.context().clone();
    thread::scope(|scope| {
        let (tx, rx) = mpsc::sync_channel(4);
        let reader_thread = scope.spawn(move || {
            loop {
                let request = protocol::read_request(&mut reader, device_id);
                let failed = request.is_err();
                if tx.send(request).is_err() {
                    break;
                }
                // Wake the event owner instead of waiting for the polling timeout
                // on every USB/IP request, especially serialized bulk reads.
                reader_context.interrupt_handle_events();
                if failed {
                    break;
                }
            }
        });
        let result = run(&mut device, &mut stream, rx, stopped);
        // Unblock the reader on every exit, including SIGTERM and USB removal.
        _ = stream.shutdown(Shutdown::Both);
        reader_thread
            .join()
            .map_err(|_| anyhow::anyhow!("USB/IP reader thread panicked"))?;
        result
    })
}

fn run(
    device: &mut UsbDevice,
    stream: &mut UnixStream,
    requests: mpsc::Receiver<anyhow::Result<Request>>,
    stopped: &AtomicBool,
) -> anyhow::Result<()> {
    let pending: RefCell<HashMap<u32, Transfer>> = RefCell::new(HashMap::new());
    let context = device.handle.context().clone();
    let mut cleanup = Deferred::new();
    cleanup.add(|| {
        let mut pending = pending.borrow_mut();
        for transfer in pending.values() {
            if !transfer.completed.get()
                && let Err(error) = transfer.cancel()
                && error != rusb::Error::NotFound
            {
                eprintln!("Cancel USB transfer: {error}");
            }
        }
        // libusb requires cancellation completion before buffers/handles are freed.
        while !pending.is_empty() {
            if let Err(error) = context.handle_events(Some(EVENT_INTERVAL))
                && error != rusb::Error::Interrupted
            {
                eprintln!("Drain USB transfers: {error}");
            }
            pending.retain(|_, transfer| !transfer.completed.get());
        }
    });

    while !stopped.load(Ordering::Relaxed) {
        let completions: Vec<_> = pending
            .borrow_mut()
            .extract_if(|_, t| t.completed.get())
            .collect();
        for (_, mut transfer) in completions {
            if transfer.write_completion(stream)? {
                return Ok(());
            }
        }
        // Bound each batch so a busy peer cannot starve USB completions or signals.
        for _ in 0..16 {
            let request = match requests.try_recv() {
                Ok(Ok(request)) => request,
                Ok(Err(error)) => {
                    if error.downcast_ref::<std::io::Error>().is_some_and(|e| {
                        matches!(
                            e.kind(),
                            std::io::ErrorKind::UnexpectedEof | std::io::ErrorKind::ConnectionReset
                        )
                    }) {
                        return Ok(());
                    }
                    return Err(error);
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => return Ok(()),
            };
            let mut pending = pending.borrow_mut();
            match request {
                Request::Unlink { header, target } => {
                    if let Some(transfer) = pending.get_mut(&target) {
                        if transfer.unlink.is_some() {
                            protocol::write_unlink(stream, header, 0)?;
                        } else {
                            // Like Linux's stub_rx: a failed/racing unlink still
                            // waits for the original completion before replying.
                            transfer.unlink = Some(header);
                            if let Err(error) = transfer.cancel()
                                && error != rusb::Error::NotFound
                            {
                                eprintln!("Cancel USB transfer: {error}");
                            }
                        }
                    } else {
                        protocol::write_unlink(stream, header, 0)?;
                    }
                }
                Request::Submit(request) => {
                    let sequence = request.header.sequence;
                    if pending.contains_key(&sequence) {
                        bail!("Duplicate USB/IP transfer sequence: {sequence}");
                    }
                    let bytes: usize = pending.values().map(|t| t.buffer.len()).sum();
                    let preparation = if pending.len() >= MAX_PENDING_TRANSFERS
                        || bytes + request.buffer.len() > MAX_PENDING_BYTES
                    {
                        Err(rusb::Error::NoMem)
                    } else if !request.iso.is_empty() {
                        Err(rusb::Error::NotSupported)
                    } else if request.header.endpoint == 0 {
                        let changing_interface = request.setup[0] == 1 && request.setup[1] == 11;
                        let busy = pending.values().any(|t| {
                            !changing_interface || t.interface.is_none_or(|i| i == request.setup[4])
                        });
                        device.control(request.setup, busy).map(|handled| {
                            if handled {
                                None
                            } else {
                                Some((
                                    TransferKind::Control,
                                    (request.setup[0] & 0x1f == 1).then_some(request.setup[4]),
                                ))
                            }
                        })
                    } else {
                        let address =
                            request.header.endpoint | if request.header.input { 0x80 } else { 0 };
                        device.endpoint(address).and_then(|ep| {
                            let kind = match ep.transfer_type {
                                TransferType::Control => TransferKind::Control,
                                TransferType::Bulk => TransferKind::Bulk,
                                TransferType::Interrupt => TransferKind::Interrupt,
                                TransferType::Isochronous => return Err(rusb::Error::NotSupported),
                            };
                            Ok(Some((kind, Some(ep.interface))))
                        })
                    };
                    match preparation {
                        Ok(Some((kind, interface))) => {
                            let mut transfer = Transfer::new(device, request, kind, interface)
                                .context("Allocate USB transfer")?;
                            match transfer.start() {
                                Ok(()) => {
                                    pending.insert(sequence, transfer);
                                }
                                Err(error) => protocol::write_submit(
                                    stream,
                                    &transfer.request,
                                    error_status(error),
                                    &[],
                                    0,
                                )?,
                            }
                        }
                        Ok(None) => protocol::write_submit(stream, &request, 0, &[], 0)?,
                        Err(error) => {
                            protocol::write_submit(stream, &request, error_status(error), &[], 0)?
                        }
                    }
                }
            }
        }
        match context.handle_events(Some(EVENT_INTERVAL)) {
            Ok(()) | Err(rusb::Error::Interrupted) => {}
            Err(error) => return Err(error).context("Handle USB transfer events"),
        }
    }
    Ok(())
}
