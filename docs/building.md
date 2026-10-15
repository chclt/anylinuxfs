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

## Experimental MTP mounts

```sh
bin/anylinuxfs list --usb
bin/anylinuxfs mount mtp:BUS_ID
bin/anylinuxfs mount mtp:BUS_ID -o rw
bin/anylinuxfs unmount mtp:BUS_ID --wait-for-vm
```

Use the current bus ID of an unlocked device in file-transfer/MTP mode. This
source selects a Linux guest and skips block-device discovery, decryption and
volume management. It mounts through jmtpfs and uses the existing NFS export,
status, unmount and stop commands. Mounts default to read-only; `-o rw` explicitly
enables experimental writes. Only `ro` and `rw` mount options are accepted.
The chosen mode applies to FUSE, the NFS export and the macOS mount, including
when NFS options are overridden. Files and directories are presented with the
invoking user's UID and GID; MTP does not store Unix ownership or permissions.
Disk driver overrides and custom actions cannot accompany an MTP source.
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
  and `scripts/jmtpfs-stable-files.patch`, `scripts/jmtpfs-writeback.patch`,
  `scripts/jmtpfs-file-operations.patch`, `scripts/jmtpfs-directory-transactions.patch`
  and `scripts/jmtpfs-fuse-open-handles.patch`
  into the same directory in that rootfs
  and run `bin/anylinuxfs shell --rw-rootfs --command
  'sh /prepare-mtp-guest.sh'`. It builds a pinned, checksum-verified
  jmtpfs revision and installs its Linux runtime libraries and license. The patched
  executable is installed at `/usr/local/libexec/anylinuxfs/jmtpfs-writeback-v1`;
  both mount modes use this versioned helper. Existing images with only the old
  `/usr/local/bin/jmtpfs` need preparation again before mounting. A rootfs
  reinitialization removes this optional helper, so prepare it again afterwards.

The patch gives each FUSE open a stable file handle. Opens of the same file share
one local copy, including when an upload changes its remote object ID. `fsync`
and `flush` synchronize without closing that copy; release and destruction only
release resources. The writeback patch stores named copies and recovery records
in `mtp-writeback` inside the VM's private cache. It uploads a temporary object,
renames the old object to a backup, publishes the replacement and only then deletes
the backup. A failed or interrupted operation retains its data and phase record;
repeated sync calls do not restart an uncertain replacement. This sequence does
not provide atomic replacement against other clients of the phone. Missing
filename-property support returns an unsupported-operation error. Upload preparation
determines the content's file type before capability checks. Writeback checks that
type and the existing destination's type before uploading or renaming any object;
the upload step does not change the prepared type. The device layer reads back the
complete uploaded metadata; successful writeback caches that snapshot rather than
the upload parameters, which libmtp does not fully populate.
Creation and file rename use the same writeback record. A rename publishes the
replacement before removing its source, and backs up an existing destination
before replacing it. Rename preflight leaves the source unchanged on failure;
namespace intent is never stored as a retryable content write. The helper disables
libfuse hidden renames and owns open-file removal itself. Unlinked or overwritten
open files keep their metadata and contents in the shared local copy until the
last handle closes; writes and sync on those handles never recreate a phone object.
An uncertain destination removal retains its recovery record and blocks writeback.
The helper statically links a pinned libfuse 2.9.9 with a private open-handle patch.
Linux can request attributes of an unlinked open inode without a file handle;
the patch routes that request to a remaining open handle. It relies on the helper's
single-threaded FUSE loop and leaves the guest's system FUSE library unchanged.
Append and truncate operate on the shared local copy. Cross-storage renames return
`EXDEV`. Directory rename and move use native MTP operations, preserving child
object IDs. Moving and renaming a directory together stages it under a unique
name to avoid collisions in the new parent. Replacing an empty directory first
backs it up and removes that backup only after publication. These multi-step
operations have independent recovery records and block further mutations of
affected directories after an uncertain result. Record persistence is shared
with file writeback; directory transactions do not use file-content caches.
Unsupported native move/filename capabilities are rejected before mutation.
Nonempty directory targets return `ENOTEMPTY`.

All MTP software runs in Linux; no host MTP package is needed. jmtpfs downloads
an entire file before serving its contents. Each MTP VM receives a private disk
cache at `/tmp/anylinuxfs-mtp-<id>` through an independent writable virtiofs mount;
its rootfs stays read-only. Guest writable-directory initialization selects that cache for `/tmp` before
network and NFS setup, because musl's `tmpfile()` does not honor `TMPDIR`. This keeps large files out of guest RAM.
The cache contains temporary plaintext copies and needs enough host disk space
for the open files. After VM exit, including startup failure and force stop, the
host removes it unless recovery records remain. In that case it reports the path
and preserves the cache. Records contain the source, target and replacement IDs,
storage and parent IDs, operation and phase, and hex-encoded source/target/staging names.
File data shares its record's basename. Directory records carry object IDs and
names without a data copy. Recovery is manual in this phase.
The tested phone reassigned object IDs after USB reconnection. Resolve current
objects using the exact recorded names and device metadata during recovery;
recorded IDs describe the interrupted session and must not be replayed blindly.
An abrupt host crash can also leave the directory behind.

A separate writable guest mount was checked with the Android phone (18d1:4ee2,
MTP+ADB), which reported support for object moves and filename properties.
Creation, overwrite, append, truncate, file rename/replacement, open-file unlink
and replacement, directory move/rename and empty-directory replacement passed.
A 557,056-byte file was closed, downloaded again and compared byte for byte.
For the disconnect check, a temporary diagnostic paused writeback after upload,
before backing up the original test file. Physical USB unplug then made sync
return `EIO`; the local replacement and phase record remained intact. After
reconnection the original file's contents were unchanged. The precise temporary
object name was also checked by creating it and reopening the MTP session; its
contents matched. These checks used a dedicated temporary phone directory,
which was removed afterwards.

The formal macOS NFS path was also checked as the invoking user, without
`--ignore-permissions`. File creation, overwrite, append, truncate, rename,
replacement and open-file unlink/replacement passed, along with directory
move/rename and empty-directory replacement. After normal unmount and startup
of a new VM, five files (including a 720,896-byte payload and a file written by
ordinary close without explicit sync) matched their expected SHA-256 hashes.
The dedicated phone directory was removed. Stopping USB forwarding made a
directory read return `EIO`; normal unmount then removed the VM, helper processes,
sockets and cache. Explicit `-o rw` also stayed consistent across FUSE and both
NFS layers when their overrides requested `ro`.

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
recognized without accessing the filesystem. Without `-o rw`, an explicit `rw`
NFS export override still produced a read-only mount. A 6 GiB file was read twice
with a 512 MiB guest, producing identical SHA-256 hashes. After waking the USB event
thread on newly received requests, its cache download averaged about 18 MiB/s,
up from about 2.5 MiB/s. The first complete read took 357 seconds and the second
14 seconds. This remains below the user's reported native 25 MB/s; the remaining
throughput difference has not been isolated. Other phone models still need
device testing. USB detach or device locks can cause I/O errors; unmount before
disconnecting. Writable mounts are experimental; recovery remains manual and
MTP replacement is not atomic against other clients.


The host resolves MTP options after merging command arguments and environment
variables. USB forwarding owns only its helper and socket; mount preparation owns
the MTP cache, and VM preparation selects the kernel once after device metadata
and the guest OS are known. The guest keeps disk and MTP lifecycles separate and
passes their mounted paths and effective permissions to common NFS export setup.
