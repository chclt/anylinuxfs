#!/bin/sh
# Optional MTP tools, built and installed only inside an Alpine Linux guest.
set -eu

if [ "$(uname -s)" != Linux ] || [ ! -f /etc/alpine-release ]; then
    echo "Run this script inside the Alpine guest with a writable root filesystem." >&2
    exit 1
fi

revision=928fb8f2eec34232e3b2cecc121195caa8865e15
checksum=70ab6c8ebc737ef77af316cd068ba7bed9cecbf39faa7668e9f4d102a882e9a3
fuse_revision=fuse-2.9.9
fuse_checksum=e57a24721177c3b3dd71cb9239ca46b4dee283db9388d48f7ccd256184982194
mtp_version=1.1.22
mtp_checksum=c3fcf411aea9cb9643590cbc9df99fa5fe30adcac695024442973d76fa5f87bc
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
work_dir=$(mktemp -d /tmp/alfs-jmtpfs.XXXXXX)
trap 'rm -rf "$work_dir"' EXIT
apk add --no-cache fuse libmtp libmagic libstdc++
apk add --no-cache --virtual .alfs-mtp-build build-base autoconf automake \
    pkgconf libtool gettext-dev libmtp-dev libusb-dev file-dev curl linux-headers patch
trap 'rm -rf "$work_dir"; apk del .alfs-mtp-build' EXIT
# The bounded event API is private to this helper; leave Alpine's libmtp intact.
curl -fL "https://github.com/libmtp/libmtp/releases/download/v$mtp_version/libmtp-$mtp_version.tar.gz" \
    -o "$work_dir/libmtp.tar.gz"
printf '%s  %s\n' "$mtp_checksum" "$work_dir/libmtp.tar.gz" | sha256sum -c -
tar -xzf "$work_dir/libmtp.tar.gz" -C "$work_dir"
mtp_dir="$work_dir/libmtp-$mtp_version"
cd "$mtp_dir"
patch -p1 < "$script_dir/libmtp-bounded-events.patch"
patch -p1 < "$script_dir/libmtp-close-timeout.patch"
CC=/usr/bin/gcc ./configure --disable-shared --enable-static --disable-mtpz --disable-doxygen
make -C src -j"${ALFS_MTP_JOBS:-2}"
# Keep the open-handle fix private to this helper's single-threaded FUSE loop.
curl -fL "https://codeload.github.com/libfuse/libfuse/tar.gz/refs/tags/$fuse_revision" \
    -o "$work_dir/fuse.tar.gz"
printf '%s  %s\n' "$fuse_checksum" "$work_dir/fuse.tar.gz" | sha256sum -c -
tar -xzf "$work_dir/fuse.tar.gz" -C "$work_dir"
fuse_dir="$work_dir/libfuse-$fuse_revision"
cd "$fuse_dir"
patch -p1 < "$script_dir/jmtpfs-fuse-open-handles.patch"
autoreconf -fi
CC=/usr/bin/gcc ./configure --disable-shared --enable-static \
    --disable-util --disable-example --disable-mtab
make -C lib -j"${ALFS_MTP_JOBS:-2}"
curl -fL "https://codeload.github.com/JasonFerrara/jmtpfs/tar.gz/$revision" \
    -o "$work_dir/source.tar.gz"
printf '%s  %s\n' "$checksum" "$work_dir/source.tar.gz" | sha256sum -c -
tar -xzf "$work_dir/source.tar.gz" -C "$work_dir"
cd "$work_dir/jmtpfs-$revision"
patch -p1 < "$script_dir/jmtpfs-stable-files.patch"
patch -p1 < "$script_dir/jmtpfs-writeback.patch"
patch -p1 < "$script_dir/jmtpfs-file-operations.patch"
patch -p1 < "$script_dir/jmtpfs-directory-transactions.patch"
patch -p1 < "$script_dir/jmtpfs-events.patch"
autoreconf -fi
# Absolute compiler paths let GCC locate cc1/cc1plus in the guest.
FUSE_CFLAGS="-I$fuse_dir/include -D_FILE_OFFSET_BITS=64" \
FUSE_LIBS="$fuse_dir/lib/.libs/libfuse.a -pthread -ldl -lrt" \
MTP_CFLAGS="-I$mtp_dir/src" \
MTP_LIBS="$mtp_dir/src/.libs/libmtp.a $(pkg-config --libs libusb-1.0)" \
CC=/usr/bin/gcc CXX=/usr/bin/g++ ./configure --prefix=/usr/local
make -j"${ALFS_MTP_JOBS:-2}"
mkdir -p /usr/local/libexec/anylinuxfs
install -m 755 src/jmtpfs /usr/local/libexec/anylinuxfs/jmtpfs-writeback-v1
mkdir -p /usr/local/share/licenses/jmtpfs
cp COPYING /usr/local/share/licenses/jmtpfs/COPYING
cp "$fuse_dir/COPYING.LIB" /usr/local/share/licenses/jmtpfs/libfuse-COPYING.LIB
cp "$mtp_dir/COPYING" /usr/local/share/licenses/jmtpfs/libmtp-COPYING
cp "$script_dir/libmtp-bounded-events.patch" /usr/local/share/licenses/jmtpfs/
cp "$script_dir/libmtp-close-timeout.patch" /usr/local/share/licenses/jmtpfs/
cp "$script_dir/jmtpfs-events.patch" /usr/local/share/licenses/jmtpfs/
cp "$script_dir/jmtpfs-fuse-open-handles.patch" /usr/local/share/licenses/jmtpfs/
cp "$script_dir/jmtpfs-stable-files.patch" /usr/local/share/licenses/jmtpfs/
cp "$script_dir/jmtpfs-writeback.patch" /usr/local/share/licenses/jmtpfs/
cp "$script_dir/jmtpfs-file-operations.patch" /usr/local/share/licenses/jmtpfs/
cp "$script_dir/jmtpfs-directory-transactions.patch" /usr/local/share/licenses/jmtpfs/
printf '%s\n' "$revision" > /usr/local/share/licenses/jmtpfs/source-revision
printf '%s\n' "$fuse_revision" > /usr/local/share/licenses/jmtpfs/libfuse-source-revision
printf '%s\n' "$mtp_version" > /usr/local/share/licenses/jmtpfs/libmtp-source-version
echo "MTP guest helper installed at /usr/local/libexec/anylinuxfs/jmtpfs-writeback-v1"
