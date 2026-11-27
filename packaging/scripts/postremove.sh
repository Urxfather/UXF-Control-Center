#!/usr/bin/env bash
set -u

# ==============================================================================
# UXF-Control-Center RPM post-removal scriptlet (%postun)
# ==============================================================================

# In RPM: $1 == 0 means full removal (erase). $1 >= 1 means upgrade/reinstall.
case "${1:-}" in
    upgrade|failed-upgrade|1|2)
        exit 0
        ;;
esac

# Full package removal only ($1 == 0):
if command -v systemctl >/dev/null 2>&1; then
    systemctl disable --now acer-control-daemon.service >/dev/null 2>&1 || true
    systemctl daemon-reload || true
fi

if command -v dkms >/dev/null 2>&1; then
    dkms remove -m acer-keyboard -v 0.1.1 --all >/dev/null 2>&1 || true
fi

if command -v udevadm >/dev/null 2>&1; then
    udevadm control --reload-rules || true
fi

if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor >/dev/null 2>&1 || true
fi
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database -q /usr/share/applications >/dev/null 2>&1 || true
fi

exit 0
