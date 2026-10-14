#!/bin/sh
# Build an experimental kernel from an already patched libkrunfw kernel tree.
# Run in Linux with a clean source tree, the libkrunfw base config, and an output directory.
# Alpine build prerequisites: build-base coreutils bash flex bison perl elfutils-dev
# openssl-dev bc linux-headers xz tar.
set -eu

if [ "$#" -ne 3 ]; then
    echo "Usage: $0 PATCHED_KERNEL_SOURCE BASE_CONFIG OUTPUT_DIRECTORY" >&2
    exit 2
fi
if [ "$(uname -s)" != Linux ] || [ "$(uname -m)" != aarch64 ]; then
    echo "Build this kernel inside an aarch64 Linux guest." >&2
    exit 1
fi

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
source_dir=$(CDPATH= cd -- "$1" && pwd)
base_config=$2
mkdir -p -- "$3"
output_dir=$(CDPATH= cd -- "$3" && pwd)
if [ "$source_dir" = "$output_dir" ]; then
    echo "Use a separate output directory to preserve the source configuration." >&2
    exit 1
fi
if [ ! -f "$base_config" ]; then
    echo "The libkrunfw base configuration does not exist: $base_config" >&2
    exit 1
fi

"$source_dir/scripts/kconfig/merge_config.sh" -m -O "$output_dir" \
    "$base_config" "$script_dir/linux-usbip.config"
make -C "$source_dir" O="$output_dir" olddefconfig
for option in USB_SUPPORT USB USBIP_CORE USBIP_VHCI_HCD FUSE_FS; do
    if ! grep -qx "CONFIG_${option}=y" "$output_dir/.config"; then
        echo "Kernel configuration did not enable CONFIG_${option}." >&2
        exit 1
    fi
done
make -C "$source_dir" O="$output_dir" -j"${ALFS_KERNEL_JOBS:-4}" Image
echo "Experimental USB/IP kernel: $output_dir/arch/arm64/boot/Image"
