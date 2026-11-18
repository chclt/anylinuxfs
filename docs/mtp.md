# Experimental MTP mounts

This branch uses AFT inside a Linux microVM and exports its FUSE filesystem
through standard kernel NFS. The Mac needs neither macFUSE nor File Provider.
USB forwarding is generic; AFT owns MTP operations, and vmproxy owns mounting,
NFS export setup and shutdown. There is no background upload queue or persistent
file-data cache in the host integration.

## Build and prepare

Run `./build-app.sh` as described in [building.md](building.md). It also builds
`libexec/usbproxy`. MTP needs a USB/IP-enabled guest kernel and an AFT helper.

The tested USB/IP kernel baseline is Linux 6.12.62, libkrunfw tag
`v6.12.62-rev2`, commit `71705c766f0f3fb7d347e3ba0d1a316249cef4dd`.
Apply that revision's `patches/0*.patch` to a clean Linux 6.12.62 tree, then run
`scripts/build-usbip-kernel.sh SOURCE BASE_CONFIG OUTPUT` inside aarch64 Linux
with its `config-libkrunfw_aarch64` (16K pages). Install the resulting
`arch/arm64/boot/Image` as `libexec/Image-usbip`. A separate 4K build goes at
`libexec/Image-usbip-4K`. Ordinary disk mounts retain their existing kernels.
Kernel build dependencies: build-base, coreutils, bash, flex, bison, perl,
elfutils-dev, openssl-dev, bc, linux-headers, xz and tar.

For the default Alpine rootfs location, prepare AFT with:

```sh
./bin/anylinuxfs shell --command true
mtp_build_dir="$HOME/.anylinuxfs/alpine/rootfs/opt/alfs-aft-build"
mkdir -p "$mtp_build_dir"
cp scripts/prepare-mtp-guest.sh scripts/aft-filesystem.patch scripts/aft-owner.patch scripts/aft-usb.patch scripts/aft-metadata.patch "$mtp_build_dir/"
./bin/anylinuxfs shell --rw-rootfs --command 'sh /opt/alfs-aft-build/prepare-mtp-guest.sh'
```

The guest script downloads AFT v4.6 at commit
`712dce6ea71793f6f8454e8ce75a0fd4ea553226`, verifies its archive SHA-256,
applies the patches and installs `/usr/local/libexec/anylinuxfs/aft-mtp-mount`.
It installs runtime dependencies and removes its temporary build dependencies.
`ALFS_AFT_ARCHIVE` optionally supplies a guest-local archive with the same
required checksum. Existing jmtpfs helpers are not used.

Changing Git branches does not replace ignored binaries or installed guest
helpers. Rebuild and prepare the corresponding helper before testing a branch.

## Mount and unmount

Unlock the phone, select file transfer and accept phone/Mac access prompts.

```sh
./bin/anylinuxfs list --usb
sudo ./bin/anylinuxfs mount mtp:BUS_ID -o rw -w false
./bin/anylinuxfs unmount mtp:BUS_ID -w
```

Replace `BUS_ID` with the current USB identifier. Without `-o rw`, the mount is
read-only. `-w false` prevents opening Finder automatically. The invoking user's
UID/GID are passed to AFT, so ordinary use does not require `--ignore-permissions`.

macOS MTP mounts default to synchronous NFS3 requests, 1 MiB reads/writes and
`dumbtimer`, retaining the ordinary bounded soft/intr retry settings. The guest
uses four NFS workers and sets the maximum block size before starting them.
No global host NFS sysctl is changed. User NFS overrides can change these defaults.
The setup uses the [Linux nfsd control filesystem](https://github.com/torvalds/linux/blob/v6.12/fs/nfsd/nfsctl.c).

The host copies `/etc/localtime` into a private temporary directory. The guest
mounts it read-only and supplies its TZif file through AFT's `TZ` environment.
Unzoned phone timestamps therefore use the computer's timezone rules, including
DST. Explicit timestamp offsets are respected. Neither the phone's settings nor
the guest's system timezone is changed. The resource directory is removed after
VM exit; it contains no phone file data.

## Patches and limitations

`aft-filesystem.patch` contains the previously isolated directory-error caching,
disconnect/close handling, on-demand metadata refresh, stable directory cookies,
FUSE export support and date parsing fixes. `aft-owner.patch` adds standard
`uid`/`gid` mount-option handling for presentation through NFS. `aft-usb.patch`
uses 64 KiB Linux USB bulk-write requests to reduce synchronous USB/IP round
trips; reads retain their 4 KiB request size. Each USB write still completes
before the next is submitted, and file writes retain their existing completion
semantics.

The host USB bridge forwards a zero-length bulk write as one terminating packet.
It only maps the guest's append-zero-packet flag to libusb for nonempty writes;
otherwise Darwin can send a second terminating packet and stall the session.

`aft-metadata.patch` makes MTP mounts content-transfer volumes. Creating entries
named `.DS_Store` or starting with `._`, including renaming to those names,
returns `EACCES` before any phone-side change. Other dotfiles such as `.gitignore`
and `.nomedia` work normally. Existing sidecars remain accessible, including
reading and deletion; they are not hidden or automatically removed. This policy
belongs to the AFT filesystem adapter and does not change USB forwarding, other
NFS exports or global Finder preferences.

- This remains experimental. MTP does not provide full POSIX semantics or atomic
  replacement, and interrupted writes can leave partial files on the phone.
- Cross-directory rename returns `EXDEV`; a client may implement moving as copy
  followed by deletion.
- Phone-restricted directories remain inaccessible. An enumeration error is not
  cached as an empty directory.
- Phone-side changes are rechecked on access after a short interval. Finder/NFS
  caches add their own delay; this is not an immediate event-based mirror.
- Original modification times may not be writable. NFS3 has no independent birth
  time field, so Finder creation dates can be absent or misleading.
- Accepted limitation: an `EndEditObject` failure during FUSE `release` may not
  reach Finder. In the examined Android implementation, including the Samsung
  Android 16 system library, successful writes have already written the file
  contents; ending the edit rescans media metadata and releases the file handle.
  This failure alone does not indicate incomplete contents, but can leave media
  information or edit-session cleanup unfinished. Per-write edit finalization
  was evaluated and deliberately not adopted; see the research record below.
- A phone may expose no storage while locked or require USB mode selection again
  after reconnecting. The application does not change that setting automatically.
- Normal unmount should precede unplugging. Automatic unmount on unplug is not
  implemented.
- New AppleDouble sidecars are not saved. Finder can report a successful copy
  while skipping tags, resource forks and other Mac extended metadata. The data
  fork, including embedded metadata such as photo EXIF, is transferred unchanged,
  but this is not a full-fidelity Mac backup destination.

See [the research index](mtp-research.md) for archived alternatives and
[`research/mtp/aft-vm`](../research/mtp/aft-vm/README.md) for the experimental
evidence and its device/version limits. Historical results are not a substitute
for regression of this integrated build.

## Integration validation (2026-11-17)

The formal CLI path built successfully for the macOS host and both guest targets;
all 96 existing Rust tests passed. On Samsung Z Fold 6, ordinary-user writes,
empty and Unicode-named files, rename, overwrite, truncate, read-only rejection,
normal unmount and direct remount passed. A 478,481,295-byte video took 37.8 s to
write while directory queries peaked at 0.42 s. Fresh-VM readback took 66.9 s and
matched SHA-256. All four test files were verified and removed.

This was a CLI regression through gvproxy, not a new Finder UI or physical
unplug test. Other phone models, phone-side edits of existing file contents and
ordinary disk integration tests remain outside this round. The phone-restricted
VLC medialib directory continued to return its known enumeration error.

## Upload and USB boundary validation (2026-11-18)

On Samsung Z Fold 6, the same 478,481,295-byte video copied through synchronous
NFS3 with gvproxy in 36.84 s with 4 KiB USB writes and 23.94 s with 64 KiB writes
(13.0 to 20.0 decimal MB/s). Both copies matched SHA-256. Direct AFT protocol
uploads improved from 16.2 to 27.1 MB/s for whole-object transfer and from 14.4
to 22.8 MB/s for chunked writes that extend the object. All six protocol uploads
were read back byte-for-byte and removed. These are controlled copy/protocol
measurements, not Finder throughput or results for other phone models.

Both USB request sizes initially timed out writing 65,524 bytes. After fixing
the host's zero-length packet flag conversion, both passed writes and readback
at 0, 1, 4,096, 65,524, 65,536, 65,537 and 131,060 bytes. Traced zero-length bulk
writes completed in 0–1 ms, and all dedicated test data was removed. This round
did not repeat physical-unplug tests.

## Finder metadata policy validation (2026-11-18)

On Samsung Z Fold 6, Finder copied plain files, tagged files, a file with a
resource fork and a nested directory, and replaced existing test files without
an error dialog. All four destination data-fork SHA-256 values matched their
sources. No new `.DS_Store` or `._*` appeared in the destination. The source tag
and resource fork were absent after copying, and Finder displayed no warning
about this loss.

Existing `.DS_Store` and `._*` fixtures remained readable and deletable. Direct
NFS calls rejected the reserved create/rename targets with `EACCES`; ordinary
dotfiles and ordinary rename worked. All phone test fixtures were removed. The
production patch retains this tested policy and removes only diagnostic logging.
