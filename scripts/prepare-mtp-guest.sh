#!/bin/sh
# Optional MTP tools, built and installed only inside an Alpine Linux guest.
set -eu

if [ "$(uname -s)" != Linux ] || [ ! -f /etc/alpine-release ]; then
    echo "Run this script inside the Alpine guest with a writable root filesystem." >&2
    exit 1
fi

revision=928fb8f2eec34232e3b2cecc121195caa8865e15
checksum=70ab6c8ebc737ef77af316cd068ba7bed9cecbf39faa7668e9f4d102a882e9a3
work_dir=$(mktemp -d /tmp/alfs-jmtpfs.XXXXXX)
trap 'rm -rf "$work_dir"' EXIT
apk add --no-cache fuse libmtp libmagic libstdc++
apk add --no-cache --virtual .alfs-mtp-build build-base autoconf automake \
    pkgconf fuse-dev libmtp-dev file-dev curl linux-headers
trap 'rm -rf "$work_dir"; apk del .alfs-mtp-build' EXIT
curl -fL "https://codeload.github.com/JasonFerrara/jmtpfs/tar.gz/$revision" \
    -o "$work_dir/source.tar.gz"
printf '%s  %s\n' "$checksum" "$work_dir/source.tar.gz" | sha256sum -c -
tar -xzf "$work_dir/source.tar.gz" -C "$work_dir"
cd "$work_dir/jmtpfs-$revision"
autoreconf -fi
# Absolute compiler paths let GCC locate cc1/cc1plus in the guest.
CC=/usr/bin/gcc CXX=/usr/bin/g++ ./configure --prefix=/usr/local
make -j"${ALFS_MTP_JOBS:-2}"
make install
mkdir -p /usr/local/share/licenses/jmtpfs
cp COPYING /usr/local/share/licenses/jmtpfs/COPYING
printf '%s\n' "$revision" > /usr/local/share/licenses/jmtpfs/source-revision
echo "MTP guest helper installed at /usr/local/bin/jmtpfs"
