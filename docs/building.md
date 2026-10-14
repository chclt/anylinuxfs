# Build from source

## Build dependencies

You need Go and Rust toolchains (not necessarily installed via Homebrew) and a few additional tools for cross-compiling the Linux helper running in the VM:

```sh
brew install go rustup
brew install lld llvm pkgconf
```

If you just installed `rustup` from Homebrew, initialise it first:

```sh
rustup default stable
export PATH="$PATH:/opt/homebrew/opt/rustup/bin"
```

## Runtime dependencies

You should already have these if you installed `anylinuxfs` from Homebrew:

```sh
brew install util-linux
```

## Building

```sh
git clone https://github.com/nohajc/anylinuxfs.git
cd anylinuxfs
rustup target add aarch64-unknown-linux-musl
rustup +nightly-2026-01-25 component add rust-src
./download-dependencies.sh
./build-app.sh             # debug build
./build-app.sh --release   # release build
```

The compiled executable will be available under `./bin`:

```sh
bin/anylinuxfs list
```

## Experimental USB forwarding helper

The build also places `usbproxy` under `libexec`. It forwards USB requests to
the host device; filesystem protocols such as MTP remain inside the Linux guest.
The USB library is compiled into the helper, so it requires no host MTP package
or separate libusb installation.

```sh
libexec/usbproxy list
libexec/usbproxy list --json
libexec/usbproxy serve --device BUS_ID --socket /tmp/anylinuxfs-usb.sock
```

Use a bus ID from the current device list. The helper runs in the foreground,
prints a JSON readiness record, and accepts one USB/IP import session at a time
on a Unix socket owned by the invoking user with mode `0600`. The VM's USB/IP
vsock port can be mapped directly to this socket. No TCP relay is needed.
An advisory device lock prevents two helpers from exporting the same bus ID.
Existing lock files retain their contents and permissions; symbolic links and
non-regular files are rejected.

Control, bulk and interrupt transfers are supported, including cancellation,
endpoint ownership and alternate settings. Isochronous requests receive an
unsupported-operation response. Interfaces are claimed when used; host kernel
drivers are not automatically detached. Closing the session cancels pending
transfers and releases the interfaces. SIGINT/SIGTERM also removes the socket.
Removal of the selected device closes the session and exits the helper.

The host commands can manage the helper and map its socket to guest vsock port
12701:

```sh
bin/anylinuxfs list --usb
bin/anylinuxfs shell --usb-device BUS_ID
bin/anylinuxfs mount DISK_IDENT --usb-device BUS_ID
```

USB forwarding currently requires a Linux guest. `--usb-device` establishes the
transport only: it does not attach the device to VHCI or mount MTP automatically.
It can accompany an existing disk mount or a diskless custom action with an NFS
export override. The host keeps the helper alive until the VM exits and cleans
up its process and socket on shutdown. USB helper diagnostics are stored in
`anylinuxfs_usb-<id>.log` beside the other application logs.

Both `--usb-device` and `mtp:BUS_ID` select `libexec/Image-usbip`, or
`Image-usbip-4K` for 4K pages, and report a missing kernel before starting the
USB helper. Prepare these kernels as described below. Mounts without USB
forwarding still use the normal bundled kernels. To build or test the helper independently:

```sh
cargo build --manifest-path usbproxy/Cargo.toml --locked
cargo test --manifest-path usbproxy/Cargo.toml --locked
```

Bundled USB library licenses are copied to `libexec/licenses/usbproxy`. The
locked Cargo dependencies include the libusb source used to rebuild the helper.

## Experimental read-only MTP mounts

```sh
bin/anylinuxfs list --usb
bin/anylinuxfs mount mtp:BUS_ID
bin/anylinuxfs unmount mtp:BUS_ID --wait-for-vm
```

Use the current bus ID of an unlocked device in file-transfer/MTP mode. This
source selects a Linux guest and skips block-device discovery, decryption and
volume management. It mounts through jmtpfs and uses the existing NFS export,
status, unmount and stop commands. This first implementation is read-only,
including when NFS export or host mount options are overridden. Disk mount
options, driver overrides and custom actions cannot accompany an MTP source.
`--usb-device` remains a transport-only option for shell and disk mounts.

Prepare these experimental guest assets before mounting:

- Build an arm64 kernel from the same patched libkrunfw source and base config as
  the bundled kernel using `scripts/build-usbip-kernel.sh`. The current baseline
  is Linux 6.12.62, libkrunfw tag `v6.12.62-rev2`, commit
  `71705c766f0f3fb7d347e3ba0d1a316249cef4dd`, and its
  `config-libkrunfw_aarch64` (16K pages). Apply that revision's `patches/0*.patch`
  to a clean Linux 6.12.62 source tree before running the script. Place the resulting
  `arch/arm64/boot/Image` at `libexec/Image-usbip`. A build using the 4K-page base
  config goes at `libexec/Image-usbip-4K` and is selected by
  `--kernel-page-size 4k`. Ordinary disk mounts still use the bundled kernels.
- First run `bin/anylinuxfs shell --command 'true'` to initialize or update the
  Alpine rootfs to the current version. Then copy `scripts/prepare-mtp-guest.sh`
  into that rootfs and run `bin/anylinuxfs shell --rw-rootfs --command
  'sh /prepare-mtp-guest.sh'`. It builds a pinned, checksum-verified
  jmtpfs revision and installs its Linux runtime libraries and license. A rootfs
  reinitialization removes this optional helper, so prepare it again afterwards.

All MTP software runs in Linux; no host MTP package is needed. jmtpfs downloads
an entire file before serving its contents. Each MTP VM receives a private disk
cache at `/tmp/anylinuxfs-mtp-<id>` through an independent writable virtiofs mount;
its rootfs stays read-only. Guest writable-directory initialization selects that cache for `/tmp` before
network and NFS setup, because musl's `tmpfile()` does not honor `TMPDIR`. This keeps large files out of guest RAM.
The cache contains temporary plaintext copies and needs enough host disk space
for the open files. It is removed after VM exit, including startup failure and
force stop. An abrupt host crash can leave the directory behind.

On macOS, MTP mounts keep soft, interruptible NFS requests but default to
`deadtimeout=0,timeo=600,retrans=100`, allowing the first read to wait for the
whole-file cache. Ordinary disk mounts keep their existing timeout options.
`--nfs-options` can override these waiting options. macOS may still report an
interrupted server connection during a long cache fill; the notification alone
does not identify a failed transfer.

The 16K kernel and guest path have been checked with a simulated USB/IP MTP
device and an Android phone (18d1:4ee2, MTP+ADB), including enumeration, NFS
directory access, repeated file reads (2.3 MiB and 32 MiB), read-only write
rejection and normal shutdown. Reads up to 87 MiB also completed. The phone
passed 12 native interrupt-transfer cancellations and disconnecting the USB/IP
client with a pending transfer.
Physical unplug during file access returned an I/O error, and the CLI then
unmounted normally and cleaned up the VM, helpers, sockets and cache. Reconnecting
the phone restored mounting and file reads, with the same file checksum.
Mount status uses the kernel's mount table so a disconnected NFS share is still
recognized without accessing the filesystem. An explicit `rw` NFS export
override still produced a read-only mount. A 6 GiB file was read twice with a
512 MiB guest, producing identical SHA-256 hashes. After waking the USB event
thread on newly received requests, its cache download averaged about 18 MiB/s,
up from about 2.5 MiB/s. The first complete read took 357 seconds and the second
14 seconds. This remains below the user's reported native 25 MB/s; the remaining
throughput difference has not been isolated. Other phone models still need
device testing. USB detach or device locks can cause I/O errors; unmount before
disconnecting. MTP writes are not implemented in this phase.


The host resolves MTP options after merging command arguments and environment
variables. USB forwarding owns only its helper and socket; mount preparation owns
the MTP cache, and VM preparation selects the kernel once after device metadata
and the guest OS are known. The guest keeps disk and MTP lifecycles separate and
passes their mounted paths and effective permissions to common NFS export setup.
