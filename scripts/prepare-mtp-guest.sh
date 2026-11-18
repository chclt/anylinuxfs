#!/bin/sh
# Build the pinned AFT helper inside a writable Alpine guest.
set -eu

if [ "$(uname -s)" != Linux ] || [ ! -f /etc/alpine-release ]; then
    echo "Run this script inside the Alpine guest with a writable root filesystem." >&2
    exit 1
fi

export PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

revision=712dce6ea71793f6f8454e8ce75a0fd4ea553226
checksum=03ba4b3bc1e31e5656fdf63b213ca259323c0fbb4b187843ce19209338ae63fa
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
# Compilation needs disk space; the guest's /tmp is a memory-backed filesystem.
mkdir -p /var/tmp
work_dir=$(mktemp -d /var/tmp/alfs-aft.XXXXXX)
trap 'rm -rf "$work_dir"' EXIT
apk add --no-cache fuse3 libstdc++
apk add --no-cache --virtual .alfs-aft-build \
    build-base cmake pkgconf fuse3-dev readline-dev linux-headers curl patch
trap 'rm -rf "$work_dir"; apk del .alfs-aft-build' EXIT

if [ -n "${ALFS_AFT_ARCHIVE:-}" ]; then
    cp "$ALFS_AFT_ARCHIVE" "$work_dir/source.tar.gz"
else
    curl -fL "https://codeload.github.com/whoozle/android-file-transfer-linux/tar.gz/$revision" \
        -o "$work_dir/source.tar.gz"
fi
printf '%s  %s\n' "$checksum" "$work_dir/source.tar.gz" | sha256sum -c -
tar -xzf "$work_dir/source.tar.gz" -C "$work_dir"
source_dir="$work_dir/android-file-transfer-linux-$revision"
cd "$source_dir"
patch -p1 < "$script_dir/aft-filesystem.patch"
patch -p1 < "$script_dir/aft-owner.patch"
patch -p1 < "$script_dir/aft-usb.patch"
patch -p1 < "$script_dir/aft-metadata.patch"
patch -p1 < "$script_dir/aft-harmony.patch"
/usr/bin/cmake -S . -B "$work_dir/build" \
    -DCMAKE_C_COMPILER=/usr/bin/gcc -DCMAKE_CXX_COMPILER=/usr/bin/g++ \
    -DBUILD_QT_UI=OFF -DBUILD_PYTHON=OFF -DBUILD_TAGLIB=OFF \
    -DBUILD_MTPZ=OFF -DBUILD_FUSE=ON -DCMAKE_BUILD_TYPE=Release
/usr/bin/cmake --build "$work_dir/build" --target aft-mtp-mount -j"${ALFS_MTP_JOBS:-1}"
destination=/usr/local/libexec/anylinuxfs
mkdir -p "$destination"
install -m 755 "$work_dir/build/fuse/aft-mtp-mount" "$destination/aft-mtp-mount.new"
mv "$destination/aft-mtp-mount.new" "$destination/aft-mtp-mount"
mkdir -p /usr/local/share/licenses/anylinuxfs-aft
install -m 644 LICENSE /usr/local/share/licenses/anylinuxfs-aft/LICENSE
echo "Installed AFT $revision with anylinuxfs filesystem, ownership, USB, metadata and device compatibility patches."
