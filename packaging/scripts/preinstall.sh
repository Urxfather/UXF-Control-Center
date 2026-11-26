#!/usr/bin/env bash
set -u

# ==============================================================================
# UXF-Control-Center RPM pre-installation scriptlet (%pre)
# ==============================================================================

STATE_FILE="/run/uxf-control-center-upgrade.state"
rm -f "$STATE_FILE"

# 1. Reliable Upgrade vs Fresh Installation Detection:
# In RPM:
#   $1 == 1: Initial/Fresh package installation
#   $1 >= 2: Package upgrade of the same name (e.g. uxf-control-center upgrade)
# When replacing an older package under another name (Obsoletes: acer-control-center):
#   RPM runs %pre with $1 == 1, but the old package is currently installed in the RPM DB.
IS_UPGRADE=0
if [ "${1:-0}" -ge 2 ]; then
    IS_UPGRADE=1
elif command -v rpm >/dev/null 2>&1 && rpm -q acer-control-center >/dev/null 2>&1; then
    IS_UPGRADE=1
fi

# Only capture and restore previous service preferences during an actual upgrade/replacement:
if [ "$IS_UPGRADE" -eq 1 ] && command -v systemctl >/dev/null 2>&1; then
    WAS_ENABLED=0
    WAS_ACTIVE=0
    if systemctl is-enabled acer-control-daemon.service >/dev/null 2>&1; then
        WAS_ENABLED=1
    fi
    if systemctl is-active acer-control-daemon.service >/dev/null 2>&1; then
        WAS_ACTIVE=1
    fi
    echo "WAS_ENABLED=$WAS_ENABLED" > "$STATE_FILE"
    echo "WAS_ACTIVE=$WAS_ACTIVE" >> "$STATE_FILE"
fi

# 2. Narrowly-scoped cleanup for legacy broken acer-keyboard/0.1.0 DKMS registration
if [ -d "/var/lib/dkms/acer-keyboard/0.1.0" ]; then
    if [ -d "/usr/src/acer-keyboard-0.1.0" ] && command -v dkms >/dev/null 2>&1; then
        dkms remove -m acer-keyboard -v 0.1.0 --all >/dev/null 2>&1 || true
    fi
    # If source was already removed by earlier package changes, remove stale tree
    if [ -d "/var/lib/dkms/acer-keyboard/0.1.0" ]; then
        rm -rf "/var/lib/dkms/acer-keyboard/0.1.0"
        for link in /var/lib/dkms/acer-keyboard/kernel-*; do
            if [ -L "$link" ] && [ ! -e "$link" ]; then
                rm -f "$link"
            fi
        done
    fi
fi

# 3. Ensure system group exists for hardware access
if ! getent group acercontrol >/dev/null 2>&1; then
    if command -v groupadd >/dev/null 2>&1; then
        groupadd --system acercontrol || true
    fi
fi

exit 0
