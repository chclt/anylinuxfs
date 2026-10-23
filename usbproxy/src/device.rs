//! Host USB access and descriptor-based routing. No MTP protocol lives here.
use anyhow::Context as _;
use rusb::{ConfigDescriptor, Context, Device, DeviceHandle, TransferType, UsbContext};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

pub fn bus_id(device: &Device<Context>) -> String {
    format!("{}-{}", device.bus_number(), device.address())
}

pub fn find(context: &Context, id: &str) -> anyhow::Result<Device<Context>> {
    context
        .devices()
        .context("Enumerate USB devices")?
        .iter()
        .find(|device| bus_id(device) == id)
        .with_context(|| format!("USB device {id} not found"))
}

#[derive(Debug)]
pub struct DeviceInfo {
    pub bus_id: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub speed: rusb::Speed,
    pub bus: u8,
    pub address: u8,
    pub descriptor: rusb::DeviceDescriptor,
    pub configuration: u8,
    pub interfaces: Vec<InterfaceInfo>,
}

#[derive(Debug, Serialize)]
pub struct InterfaceInfo {
    pub number: u8,
    pub class: u8,
    pub subclass: u8,
    pub protocol: u8,
}

impl DeviceInfo {
    pub fn read(device: &Device<Context>) -> anyhow::Result<Self> {
        let descriptor = device
            .device_descriptor()
            .context("Read USB device descriptor")?;
        let (config, configuration) = match device.active_config_descriptor() {
            Ok(config) => {
                let number = config.number();
                (config, number)
            }
            Err(rusb::Error::NotFound) => (
                device
                    .config_descriptor(0)
                    .context("Read USB configuration")?,
                0,
            ),
            Err(error) => return Err(error).context("Read active USB configuration"),
        };
        let interfaces = config
            .interfaces()
            .filter_map(|interface| {
                interface
                    .descriptors()
                    .find(|d| d.setting_number() == 0)
                    .map(|d| InterfaceInfo {
                        number: d.interface_number(),
                        class: d.class_code(),
                        subclass: d.sub_class_code(),
                        protocol: d.protocol_code(),
                    })
            })
            .collect::<Vec<_>>();
        Ok(Self {
            bus_id: bus_id(device),
            vendor_id: descriptor.vendor_id(),
            product_id: descriptor.product_id(),
            speed: device.speed(),
            bus: device.bus_number(),
            address: device.address(),
            descriptor,
            configuration,
            interfaces,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Endpoint {
    pub interface: u8,
    pub transfer_type: TransferType,
}

struct Setting {
    interface: u8,
    alternate: u8,
    endpoints: Vec<(u8, Endpoint)>,
}

pub struct UsbDevice {
    pub handle: DeviceHandle<Context>,
    pub id: u32,
    settings: Vec<Setting>,
    alternates: BTreeMap<u8, u8>,
    claimed: BTreeSet<u8>,
}

impl UsbDevice {
    pub fn open(device: &Device<Context>) -> anyhow::Result<Self> {
        let handle = device.open().context("Open USB device")?;
        crate::trace::event(format_args!("device_open bus_id={}", bus_id(device)));
        let settings = match device.active_config_descriptor() {
            Ok(config) => settings(&config),
            Err(rusb::Error::NotFound) => vec![],
            Err(error) => return Err(error).context("Read active USB configuration"),
        };
        Ok(Self {
            handle,
            id: (device.bus_number() as u32) << 16 | device.address() as u32,
            settings,
            alternates: BTreeMap::new(),
            claimed: BTreeSet::new(),
        })
    }

    fn claim(&mut self, interface: u8) -> rusb::Result<()> {
        if !self.claimed.contains(&interface) {
            // Leave kernel-owned interfaces alone; never detach host drivers automatically.
            self.handle.claim_interface(interface)?;
            self.claimed.insert(interface);
            crate::trace::event(format_args!("interface_claim interface={interface}"));
        }
        Ok(())
    }

    pub fn endpoint(&mut self, address: u8) -> rusb::Result<Endpoint> {
        let endpoint = route(&self.settings, &self.alternates, address)?;
        self.claim(endpoint.interface)?;
        Ok(endpoint)
    }

    /// Standard requests must update libusb's bookkeeping as well as the device.
    /// Returns true when the request has been handled without a raw control URB.
    pub fn control(&mut self, setup: [u8; 8], busy: bool) -> rusb::Result<bool> {
        let value = u16::from_le_bytes([setup[2], setup[3]]);
        let index = u16::from_le_bytes([setup[4], setup[5]]);
        let length = u16::from_le_bytes([setup[6], setup[7]]);
        match (setup[0], setup[1]) {
            (0, 5) if length == 0 => return Ok(true), // Guest address is virtual.
            (0, 9) if length == 0 => {
                let configuration = u8::try_from(value).map_err(|_| rusb::Error::InvalidParam)?;
                if self.handle.active_configuration()? != configuration {
                    if busy {
                        return Err(rusb::Error::Busy);
                    }
                    for interface in self.claimed.clone() {
                        self.handle.release_interface(interface)?;
                        self.claimed.remove(&interface);
                        crate::trace::event(format_args!(
                            "interface_release interface={interface}"
                        ));
                    }
                    if configuration == 0 {
                        self.handle.unconfigure()?;
                    } else {
                        self.handle.set_active_configuration(configuration)?;
                    }
                    self.settings = if configuration == 0 {
                        vec![]
                    } else {
                        settings(&self.handle.device().active_config_descriptor()?)
                    };
                    self.alternates.clear();
                }
                return Ok(true);
            }
            (1, 11) if length == 0 => {
                if busy {
                    return Err(rusb::Error::Busy);
                }
                let interface = u8::try_from(index).map_err(|_| rusb::Error::InvalidParam)?;
                let alternate = u8::try_from(value).map_err(|_| rusb::Error::InvalidParam)?;
                self.claim(interface)?;
                self.handle.set_alternate_setting(interface, alternate)?;
                self.alternates.insert(interface, alternate);
                return Ok(true);
            }
            (2, 1) if value == 0 && length == 0 => {
                let address = u8::try_from(index).map_err(|_| rusb::Error::InvalidParam)?;
                self.endpoint(address)?;
                self.handle.clear_halt(address)?;
                return Ok(true);
            }
            _ => {}
        }
        match setup[0] & 0x1f {
            1 => self.claim(u8::try_from(index & 0xff).map_err(|_| rusb::Error::InvalidParam)?)?,
            2 => {
                self.endpoint(index as u8)?;
            }
            _ => {}
        }
        Ok(false)
    }
}

impl Drop for UsbDevice {
    fn drop(&mut self) {
        // This marks the start of teardown; the DeviceHandle field releases its
        // claimed interfaces and closes the device after this method returns.
        crate::trace::event(format_args!(
            "device_close_begin claimed_interfaces={:?}",
            self.claimed
        ));
    }
}

fn route(
    settings: &[Setting],
    alternates: &BTreeMap<u8, u8>,
    address: u8,
) -> rusb::Result<Endpoint> {
    settings
        .iter()
        .filter(|s| s.alternate == *alternates.get(&s.interface).unwrap_or(&0))
        .flat_map(|s| s.endpoints.iter())
        .find(|(ep, _)| *ep == address)
        .map(|(_, endpoint)| *endpoint)
        .ok_or(rusb::Error::InvalidParam)
}

fn settings(config: &ConfigDescriptor) -> Vec<Setting> {
    config
        .interfaces()
        .flat_map(|interface| {
            interface.descriptors().map(|descriptor| Setting {
                interface: descriptor.interface_number(),
                alternate: descriptor.setting_number(),
                endpoints: descriptor
                    .endpoint_descriptors()
                    .map(|ep| {
                        (
                            ep.address(),
                            Endpoint {
                                interface: descriptor.interface_number(),
                                transfer_type: ep.transfer_type(),
                            },
                        )
                    })
                    .collect(),
            })
        })
        .collect()
}
