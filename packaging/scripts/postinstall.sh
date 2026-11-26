#!/usr/bin/env bash
set -u

# ==============================================================================
# UXF-Control-Center RPM post-installation scriptlet (%post)
# ==============================================================================

PACKAGE_NAME="acer-keyboard"
PACKAGE_VERSION="0.1.1"

# 1. Ensure user group exists
if ! getent group acercontrol >/dev/null 2>&1; then
    if command -v groupadd >/dev/null 2>&1; then
        groupadd --system acercontrol || true
    fi
fi

# 2. DKMS Lifecycle: Source registration and kernel module build
DKMS_OK=0
if command -v dkms >/dev/null 2>&1; then
    # Generate MOK if Secure Boot is enabled and keys do not exist
    if command -v mokutil >/dev/null 2>&1 && mokutil --sb-state 2>&1 | grep -qi 'SecureBoot enabled'; then
        if [[ ! -s /var/lib/dkms/mok.key || ! -s /var/lib/dkms/mok.pub ]]; then
            dkms generate_mok || echo "WARNING: DKMS could not create a signing key; Secure Boot may reject the module."
        fi
    fi

    # Register 0.1.1 source tree if not registered
    if ! dkms status -m "$PACKAGE_NAME" -v "$PACKAGE_VERSION" 2>/dev/null | grep -q .; then
        if ! dkms add -m "$PACKAGE_NAME" -v "$PACKAGE_VERSION" >/dev/null 2>&1; then
            echo "ERROR: Failed to register $PACKAGE_NAME/$PACKAGE_VERSION with DKMS."
        fi
    fi

    RUNNING_KVER="$(uname -r)"
    BUILT_RUNNING=0
    MISSING_HEADERS=0

    # Check headers for running kernel
    if [ ! -d "/lib/modules/$RUNNING_KVER/build" ]; then
        MISSING_HEADERS=1
    fi

    # Build for all kernels with headers
    for kdir in /lib/modules/*; do
        kver="$(basename "$kdir")"
        if [ -d "$kdir/build" ]; then
            if dkms install -m "$PACKAGE_NAME" -v "$PACKAGE_VERSION" -k "$kver" >/dev/null 2>&1; then
                depmod -a "$kver" 2>/dev/null || true
                if [ "$kver" = "$RUNNING_KVER" ]; then
                    BUILT_RUNNING=1
                    DKMS_OK=1
                fi
            else
                echo "WARNING: DKMS build/install failed for kernel $kver."
            fi
        fi
    done

    if [ "$BUILT_RUNNING" -eq 0 ]; then
        if [ "$MISSING_HEADERS" -eq 1 ]; then
            echo "*******************************************************************"
            echo "NOTICE: Kernel development headers for running kernel ($RUNNING_KVER) are missing."
            echo "Hardware control driver cannot be compiled until headers are installed."
            echo "Install headers with: sudo dnf install kernel-devel-matched"
            echo "Then run:             sudo dkms autoinstall -k $RUNNING_KVER"
            echo "*******************************************************************"
        else
            echo "WARNING: DKMS driver build failed for running kernel ($RUNNING_KVER)."
            echo "Check DKMS build logs under /var/lib/dkms/acer-keyboard/0.1.1/build/make.log"
        fi
    fi
else
    echo "WARNING: DKMS is not installed. Driver compilation was skipped."
fi

# 3. Reload udev rules and trigger device permissions
if command -v udevadm >/dev/null 2>&1; then
    udevadm control --reload-rules || true
    udevadm trigger --subsystem-match=misc 2>/dev/null || udevadm trigger || true
fi

# 4. Hardware-aware DMI detection and module loading
SYS_VENDOR="$(cat /sys/class/dmi/id/sys_vendor 2>/dev/null || echo '')"
BOARD_NAME="$(cat /sys/class/dmi/id/board_name 2>/dev/null || echo '')"
PROD_NAME="$(cat /sys/class/dmi/id/product_name 2>/dev/null || echo '')"

# Load core keyboard driver
modprobe acer_keyboard 2>/dev/null || true

# Determine backend based on hardware platform:
# Primary verified: AlgAl15g53 / Aspire A715-79G uses acer_acpi EC interface.
# acer_wmi is incompatible with A715-79G and returns -ENODEV.
if echo "$BOARD_NAME $PROD_NAME" | grep -qiE "AlgAl15g53|A715-79G"; then
    modprobe acer_acpi 2>/dev/null || true
    modprobe acer_io 2>/dev/null || true
else
    # On other Acer models, probe acer_acpi first, fall back to acer_wmi if acpi not supported
    if modprobe acer_acpi 2>/dev/null; then
        modprobe acer_io 2>/dev/null || true
    else
        modprobe acer_wmi 2>/dev/null || true
        modprobe acer_io 2>/dev/null || true
    fi
fi

# 5. Reload system D-Bus configuration
if command -v busctl >/dev/null 2>&1; then
    busctl call --system org.freedesktop.DBus /org/freedesktop/DBus org.freedesktop.DBus ReloadConfig >/dev/null 2>&1 || true
fi

# 6. Secure Boot MOK enrollment status check
if command -v mokutil >/dev/null 2>&1 && mokutil --sb-state 2>&1 | grep -qi 'SecureBoot enabled'; then
    if [[ -s /var/lib/dkms/mok.pub ]]; then
        if ! mokutil --test-key /var/lib/dkms/mok.pub 2>&1 | grep -qi 'already enrolled'; then
            echo "*******************************************************************"
            echo "NOTICE: Secure Boot is enabled and the DKMS signing key needs enrollment."
            echo "To complete kernel module verification, run:"
            echo "  sudo mokutil --import /var/lib/dkms/mok.pub"
            echo "Set a temporary password, reboot, and choose 'Enroll MOK' in the UEFI menu."
            echo "*******************************************************************"
        fi
    fi
fi

exit 0
