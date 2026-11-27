#!/usr/bin/env bash
set -u

# ==============================================================================
# UXF-Control-Center RPM post-transaction scriptlet (%posttrans)
# ==============================================================================
# Runs at the very end of the transaction, AFTER all packages are unpacked and
# after all obsoleted packages have been erased. This guarantees that any destructive
# actions from an obsoleted package's %postun are completely remedied.

STATE_FILE="/run/uxf-control-center-upgrade.state"

# 1. Re-apply udev rules and trigger device node permissions
if command -v udevadm >/dev/null 2>&1; then
    udevadm control --reload-rules || true
    udevadm trigger --subsystem-match=misc 2>/dev/null || udevadm trigger || true
fi

# 2. Restore administrator's service preference or enable on fresh install
if command -v systemctl >/dev/null 2>&1; then
    systemctl daemon-reload || true

    if [ -f "$STATE_FILE" ]; then
        # Upgrade path: restore administrator's previous preference
        WAS_ENABLED=0
        WAS_ACTIVE=0
        . "$STATE_FILE"
        rm -f "$STATE_FILE"

        if [ "$WAS_ENABLED" -eq 1 ]; then
            systemctl enable acer-control-daemon.service >/dev/null 2>&1 || true
        fi
        if [ "$WAS_ACTIVE" -eq 1 ]; then
            systemctl restart acer-control-daemon.service >/dev/null 2>&1 || \
            systemctl start acer-control-daemon.service >/dev/null 2>&1 || true
        fi
    else
        # Fresh install path: enable and start service automatically
        systemctl enable --now acer-control-daemon.service >/dev/null 2>&1 || true
    fi
fi

# 3. Refresh icon and desktop entry caches for GNOME Shell app grid & dock
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor >/dev/null 2>&1 || true
fi
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database -q /usr/share/applications >/dev/null 2>&1 || true
fi

exit 0
