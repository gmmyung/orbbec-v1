#!/usr/bin/env bash
set -euo pipefail
if [[ ${EUID} -ne 0 ]]; then
    echo "Run with sudo: sudo bash $0" >&2
    exit 1
fi
script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
install -m 0644 "$script_dir/99-orbbec-v1.rules" /etc/udev/rules.d/99-orbbec-v1.rules
udevadm control --reload-rules
udevadm trigger --action=change --subsystem-match=usb --attr-match=idVendor=2bc5
udevadm settle
echo "Installed Orbbec USB permissions. If access is still denied, reconnect the camera."
