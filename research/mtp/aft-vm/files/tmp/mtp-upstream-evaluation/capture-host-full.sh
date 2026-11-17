#!/bin/sh
set -eu
capture_dir=/Users/jike/Developer/chclt/anylinuxfs/tmp/mtp-upstream-evaluation/host-guest-full-01
: "${SUDO_UID:?Please run this script with sudo}"
: "${SUDO_GID:?Please run this script with sudo}"
if [ -e "$capture_dir/host.pcap" ]; then
  echo 'A capture already exists; stopped to preserve it.' >&2
  exit 1
fi
capture_pid=
cleanup() {
  trap - EXIT INT TERM HUP
  if [ -n "$capture_pid" ]; then
    kill -INT "$capture_pid" 2>/dev/null || true
    wait "$capture_pid" 2>/dev/null || true
  fi
  touch "$capture_dir/stopped"
  chown "$SUDO_UID:$SUDO_GID" "$capture_dir/stopped" "$capture_dir/host.pcap" "$capture_dir/capture.log" 2>/dev/null || true
  echo 'Host NFS capture stopped.'
}
trap cleanup EXIT INT TERM HUP
/usr/sbin/tcpdump -i lo0 -n -s 0 -B 8192 -U -w "$capture_dir/host.pcap" 'tcp port 12049' > "$capture_dir/capture.log" 2>&1 &
capture_pid=$!
sleep 1
if ! kill -0 "$capture_pid" 2>/dev/null; then
  cat "$capture_dir/capture.log" >&2
  exit 1
fi
chown "$SUDO_UID:$SUDO_GID" "$capture_dir/host.pcap" "$capture_dir/capture.log"
touch "$capture_dir/ready"
chown "$SUDO_UID:$SUDO_GID" "$capture_dir/ready"
echo 'Host NFS capture ready. Keep this terminal open; it stops automatically within 5 minutes.'
seconds_left=300
while [ "$seconds_left" -gt 0 ] && [ ! -e "$capture_dir/stop" ]; do
  sleep 1
  seconds_left=$((seconds_left - 1))
done
