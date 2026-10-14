//! USB/IP wire framing, following Linux's usbip_common.h and usbip_network.h.
use anyhow::{Context, bail};
use std::io::{Read, Write};

pub const VERSION: u16 = 0x0111;
pub const IMPORT: u16 = 0x8003;
pub const DEVLIST: u16 = 0x8005;
pub const MAX_TRANSFER: usize = 16 * 1024 * 1024;

pub fn read_operation(stream: &mut impl Read) -> anyhow::Result<u16> {
    let mut bytes = [0; 8];
    stream
        .read_exact(&mut bytes)
        .context("Read USB/IP operation")?;
    if u16::from_be_bytes([bytes[0], bytes[1]]) != VERSION || u32_at(&bytes, 4) != 0 {
        bail!("Invalid USB/IP operation header");
    }
    Ok(u16::from_be_bytes([bytes[2], bytes[3]]))
}

pub fn write_operation(stream: &mut impl Write, operation: u16, status: u32) -> anyhow::Result<()> {
    stream
        .write_all(&VERSION.to_be_bytes())
        .context("Write USB/IP version")?;
    stream
        .write_all(&(operation & 0x7fff).to_be_bytes())
        .context("Write USB/IP operation")?;
    stream
        .write_all(&status.to_be_bytes())
        .context("Write USB/IP operation status")?;
    Ok(())
}

pub fn read_bus_id(stream: &mut impl Read) -> anyhow::Result<String> {
    let mut bytes = [0; 32];
    stream
        .read_exact(&mut bytes)
        .context("Read USB/IP bus ID")?;
    let end = bytes
        .iter()
        .position(|b| *b == 0)
        .context("USB/IP bus ID is not terminated")?;
    Ok(std::str::from_utf8(&bytes[..end])
        .context("Invalid USB/IP bus ID")?
        .to_owned())
}

#[derive(Clone, Copy, Debug)]
pub struct Header {
    pub sequence: u32,
    pub input: bool,
    pub endpoint: u8,
}

#[derive(Debug)]
pub struct Submit {
    pub header: Header,
    pub flags: u32,
    pub setup: [u8; 8],
    pub buffer: Vec<u8>,
    // Keep iso descriptors so an unsupported request can still receive a
    // correctly framed error response, without losing the next request.
    pub iso: Vec<[u8; 16]>,
}

#[derive(Debug)]
pub enum Request {
    Submit(Submit),
    Unlink { header: Header, target: u32 },
}

pub fn read_request(stream: &mut impl Read, device: u32) -> anyhow::Result<Request> {
    let mut bytes = [0; 48];
    stream
        .read_exact(&mut bytes)
        .context("Read USB/IP transfer header")?;
    let direction = u32_at(&bytes, 12);
    let endpoint = u32_at(&bytes, 16);
    if u32_at(&bytes, 8) != device || direction > 1 || endpoint > 15 {
        bail!("Invalid USB/IP device, direction or endpoint");
    }
    let header = Header {
        sequence: u32_at(&bytes, 4),
        input: direction == 1,
        endpoint: endpoint as u8,
    };
    match u32_at(&bytes, 0) {
        1 => {
            let length = u32_at(&bytes, 24) as usize;
            let packets = u32_at(&bytes, 32) as usize;
            if length > MAX_TRANSFER || packets > 1024 {
                bail!("USB/IP transfer exceeds buffer or packet limit");
            }
            let mut setup = [0; 8];
            setup.copy_from_slice(&bytes[40..48]);
            if header.endpoint == 0
                && (packets != 0
                    || u16::from_le_bytes([setup[6], setup[7]]) as usize != length
                    || (length != 0 && (setup[0] & 0x80 != 0) != header.input))
            {
                bail!("Invalid USB/IP control setup");
            }
            let mut buffer = vec![0; length];
            if !header.input {
                stream
                    .read_exact(&mut buffer)
                    .context("Read USB/IP output payload")?;
            }
            let mut iso = vec![[0; 16]; packets];
            for packet in &mut iso {
                stream
                    .read_exact(packet)
                    .context("Read USB/IP iso descriptor")?;
            }
            Ok(Request::Submit(Submit {
                header,
                flags: u32_at(&bytes, 20),
                setup,
                buffer,
                iso,
            }))
        }
        2 => Ok(Request::Unlink {
            header,
            target: u32_at(&bytes, 20),
        }),
        command => bail!("Unsupported USB/IP command: {command}"),
    }
}

fn reply_header(command: u32, sequence: u32, status: i32) -> [u8; 48] {
    // Linux replies zero devid/direction/ep; the client restores them from seqnum.
    let mut bytes = [0; 48];
    bytes[..4].copy_from_slice(&command.to_be_bytes());
    bytes[4..8].copy_from_slice(&sequence.to_be_bytes());
    bytes[20..24].copy_from_slice(&status.to_be_bytes());
    bytes
}

pub fn write_submit(
    stream: &mut impl Write,
    request: &Submit,
    status: i32,
    data: &[u8],
    actual: usize,
) -> anyhow::Result<()> {
    let mut bytes = reply_header(3, request.header.sequence, status);
    bytes[24..28].copy_from_slice(&(actual as u32).to_be_bytes());
    bytes[32..36].copy_from_slice(&(request.iso.len() as u32).to_be_bytes());
    bytes[36..40].copy_from_slice(&(request.iso.len() as u32).to_be_bytes());
    stream
        .write_all(&bytes)
        .context("Write USB/IP transfer reply")?;
    if request.header.input {
        stream
            .write_all(data)
            .context("Write USB/IP input payload")?;
    }
    for descriptor in &request.iso {
        let mut descriptor = *descriptor;
        descriptor[8..12].fill(0);
        descriptor[12..16].copy_from_slice(&status.to_be_bytes());
        stream
            .write_all(&descriptor)
            .context("Write USB/IP iso error descriptor")?;
    }
    Ok(())
}

pub fn write_unlink(stream: &mut impl Write, header: Header, status: i32) -> anyhow::Result<()> {
    stream
        .write_all(&reply_header(4, header.sequence, status))
        .context("Write USB/IP unlink reply")
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

pub fn speed_value(speed: rusb::Speed) -> anyhow::Result<u32> {
    // USB/IP uses Linux USB_SPEED_*, whose SuperSpeed value is 5 (not 4).
    Ok(match speed {
        rusb::Speed::Low => 1,
        rusb::Speed::Full => 2,
        rusb::Speed::High => 3,
        rusb::Speed::Super => 5,
        rusb::Speed::SuperPlus => 6,
        _ => bail!("USB device has unknown speed"),
    })
}

pub fn write_device(
    stream: &mut impl Write,
    info: &crate::device::DeviceInfo,
    include_interfaces: bool,
) -> anyhow::Result<()> {
    let mut wire = [0; 312];
    let path = format!("usb://{}", info.bus_id);
    wire[..path.len()].copy_from_slice(path.as_bytes());
    wire[256..256 + info.bus_id.len()].copy_from_slice(info.bus_id.as_bytes());
    wire[288..292].copy_from_slice(&(info.bus as u32).to_be_bytes());
    wire[292..296].copy_from_slice(&(info.address as u32).to_be_bytes());
    wire[296..300].copy_from_slice(&speed_value(info.speed)?.to_be_bytes());
    wire[300..302].copy_from_slice(&info.vendor_id.to_be_bytes());
    wire[302..304].copy_from_slice(&info.product_id.to_be_bytes());
    let version = info.descriptor.device_version();
    let bcd = ((version.major() / 10) as u16) << 12
        | ((version.major() % 10) as u16) << 8
        | (version.minor() as u16) << 4
        | version.sub_minor() as u16;
    wire[304..306].copy_from_slice(&bcd.to_be_bytes());
    wire[306..312].copy_from_slice(&[
        info.descriptor.class_code(),
        info.descriptor.sub_class_code(),
        info.descriptor.protocol_code(),
        info.configuration,
        info.descriptor.num_configurations(),
        info.interfaces.len() as u8,
    ]);
    stream
        .write_all(&wire)
        .context("Write USB/IP device descriptor")?;
    if include_interfaces {
        for interface in &info.interfaces {
            stream
                .write_all(&[interface.class, interface.subclass, interface.protocol, 0])
                .context("Write USB/IP interface descriptor")?;
        }
    }
    Ok(())
}

// USB/IP carries Linux errno values, even when this helper runs on macOS.
pub fn error_status(error: rusb::Error) -> i32 {
    match error {
        rusb::Error::InvalidParam => -22,
        rusb::Error::Access => -13,
        rusb::Error::NoDevice => -19,
        rusb::Error::NotFound => -2,
        rusb::Error::Busy => -16,
        rusb::Error::Timeout => -110,
        rusb::Error::Overflow => -75,
        rusb::Error::Pipe => -32,
        rusb::Error::Interrupted => -4,
        rusb::Error::NoMem => -12,
        rusb::Error::NotSupported => -95,
        _ => -5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{self, Cursor};

    // Linux usbip_header: command, seqnum, devid, direction, ep, then URB fields.
    const OUT: [u8; 48] = [
        0, 0, 0, 1, 0, 0, 0, 7, 0, 1, 0, 2, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    const UNLINK: [u8; 48] = [
        0, 0, 0, 2, 0, 0, 0, 8, 0, 1, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 7, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];

    struct Fragmented(Cursor<Vec<u8>>);
    impl Read for Fragmented {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            let length = buffer.len().min(3);
            self.0.read(&mut buffer[..length])
        }
    }

    #[test]
    fn fragmented_payload_does_not_consume_next_request() {
        let mut stream = Fragmented(Cursor::new([OUT.as_slice(), b"abc", &UNLINK].concat()));
        let Request::Submit(request) = read_request(&mut stream, 0x10002).unwrap() else {
            panic!()
        };
        assert_eq!(request.buffer, b"abc");
        assert_eq!(request.header.endpoint, 2);
        let Request::Unlink { header, target } = read_request(&mut stream, 0x10002).unwrap() else {
            panic!()
        };
        assert_eq!((header.sequence, target), (8, 7));
    }

    #[test]
    fn control_setup_uses_little_endian_length() {
        let mut header = OUT;
        header[15] = 1;
        header[19] = 0;
        header[27] = 18;
        header[40..48].copy_from_slice(&[0x80, 6, 0, 1, 0, 0, 18, 0]);
        let Request::Submit(request) = read_request(&mut Cursor::new(header), 0x10002).unwrap()
        else {
            panic!()
        };
        assert_eq!(request.buffer.len(), 18);
        assert!(request.header.input);
        header[47] = 1;
        assert!(read_request(&mut Cursor::new(header), 0x10002).is_err());
    }

    #[test]
    fn invalid_lengths_and_device_ids_are_rejected_before_payload_read() {
        for length in [u32::MAX, MAX_TRANSFER as u32 + 1] {
            let mut header = OUT;
            header[24..28].copy_from_slice(&length.to_be_bytes());
            let error = read_request(&mut Cursor::new(header), 0x10002).unwrap_err();
            assert!(error.to_string().contains("limit"));
        }
        assert!(read_request(&mut Cursor::new(OUT), 0x10003).is_err());
    }

    #[test]
    fn unsupported_iso_request_preserves_stream_framing() {
        let mut header = OUT;
        header[35] = 1;
        let descriptor = [0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0];
        let mut stream = Cursor::new([header.as_slice(), b"abc", &descriptor, &UNLINK].concat());
        let Request::Submit(request) = read_request(&mut stream, 0x10002).unwrap() else {
            panic!()
        };
        let mut reply = Vec::new();
        write_submit(&mut reply, &request, -95, &[], 0).unwrap();
        assert_eq!(reply.len(), 64);
        assert_eq!(&reply[32..40], &[0, 0, 0, 1, 0, 0, 0, 1]);
        assert_eq!(&reply[60..64], &[255, 255, 255, 161]);
        assert!(matches!(
            read_request(&mut stream, 0x10002).unwrap(),
            Request::Unlink { target: 7, .. }
        ));
    }

    #[test]
    fn cancellation_reply_uses_unlink_sequence_and_linux_errno() {
        let mut reply = Vec::new();
        write_unlink(
            &mut reply,
            Header {
                sequence: 8,
                input: false,
                endpoint: 0,
            },
            -104,
        )
        .unwrap();
        assert_eq!(reply.len(), 48);
        assert_eq!(
            &reply[..24],
            &[
                0, 0, 0, 4, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 255, 152
            ]
        );
        assert!(reply[24..].iter().all(|b| *b == 0));
    }
}
