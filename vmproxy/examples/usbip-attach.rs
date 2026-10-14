// Debug attachment uses the same VHCI implementation as MTP mounts.
#[cfg(target_os = "linux")]
#[path = "../src/usbip.rs"]
mod usbip;

#[cfg(target_os = "linux")]
fn main() -> anyhow::Result<()> {
    use anyhow::Context;
    use clap::Parser;
    #[derive(Parser)]
    struct Args {
        bus_id: String,
    }
    let args = Args::parse();
    let mut deferred = common_utils::Deferred::new();
    let device = usbip::attach(&args.bus_id, &mut deferred)?;
    println!(
        "Attached as guest USB {}-{}. Press Enter to detach.",
        device.bus, device.number
    );
    std::io::stdin()
        .read_line(&mut String::new())
        .context("Wait for detach")?;
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn main() -> anyhow::Result<()> {
    anyhow::bail!("Run this example inside the Linux guest")
}
