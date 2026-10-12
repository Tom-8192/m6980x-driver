"""Raw feature-report transport for the Gigabyte GHOST M6980X.

The mouse is configured through an 8-byte HID feature report *without* a report ID
on USB interface 0 (the mouse interface itself).

* Windows / macOS: hidapi (``pip install hidapi``).
* Linux: direct ``/dev/hidraw*`` ioctls - no dependency, and unlike hidapi's libusb
  backend it never detaches the kernel driver (the mouse keeps working).
"""

import glob
import os
import sys

VID = 0xE0FF
PID = 0x0001
INTERFACE = 0
REPORT_LEN = 8  # data bytes, report ID 0 is not part of this


class TransportError(Exception):
    pass


def _strip_report_id(buf):
    """hidapi and hidraw both return [report_id, data...]; normalise to 8 data bytes."""
    buf = bytes(buf)
    if len(buf) == REPORT_LEN + 1:
        return buf[1:]
    if len(buf) == REPORT_LEN:
        return buf
    raise TransportError(f"unexpected feature report length {len(buf)}: {buf.hex(' ')}")


class HidapiTransport:
    def __init__(self, path=None):
        try:
            import hid
        except ImportError as e:
            raise TransportError("hidapi fehlt: pip install hidapi") from e
        if path is None:
            devs = [d for d in hid.enumerate(VID, PID) if d["interface_number"] == INTERFACE]
            if not devs:
                raise TransportError(f"Maus {VID:04X}:{PID:04X} (Interface {INTERFACE}) nicht gefunden")
            path = devs[0]["path"]
        self._dev = hid.device()
        self._dev.open_path(path)
        self.path = path.decode() if isinstance(path, bytes) else path

    def set_feature(self, data):
        if self._dev.send_feature_report(b"\x00" + bytes(data)) < 0:
            raise TransportError("send_feature_report fehlgeschlagen")

    def get_feature(self):
        return _strip_report_id(self._dev.get_feature_report(0, REPORT_LEN + 1))

    def close(self):
        self._dev.close()


def _ioc(direction, kind, nr, size):
    return (direction << 30) | (size << 16) | (ord(kind) << 8) | nr


_IOC_WRITE, _IOC_READ = 1, 2
HIDIOCSFEATURE = _ioc(_IOC_WRITE | _IOC_READ, "H", 0x06, REPORT_LEN + 1)
HIDIOCGFEATURE = _ioc(_IOC_WRITE | _IOC_READ, "H", 0x07, REPORT_LEN + 1)


def find_hidraw():
    """Return /dev/hidrawN of the mouse interface (USB interface number 0)."""
    hid_id = f"HID_ID=0003:{VID:08X}:{PID:08X}"
    for node in sorted(glob.glob("/sys/class/hidraw/hidraw*")):
        try:
            with open(os.path.join(node, "device", "uevent")) as f:
                if hid_id not in f.read().upper():
                    continue
        except OSError:
            continue
        # .../3-2:1.0/0003:E0FF:0001.0007 -> USB interface directory ends in ".<ifnum>"
        usb_if = os.path.basename(os.path.dirname(os.path.realpath(os.path.join(node, "device"))))
        if usb_if.endswith(f".{INTERFACE}"):
            return "/dev/" + os.path.basename(node)
    return None


class HidrawTransport:
    def __init__(self, path=None):
        import fcntl
        self._fcntl = fcntl
        self.path = path or find_hidraw()
        if not self.path:
            raise TransportError(f"Maus {VID:04X}:{PID:04X} (Interface {INTERFACE}) nicht gefunden")
        try:
            self._fd = os.open(self.path, os.O_RDWR)
        except PermissionError as e:
            raise TransportError(f"Kein Zugriff auf {self.path} - udev-Regel aus linux/ installieren") from e

    def set_feature(self, data):
        self._fcntl.ioctl(self._fd, HIDIOCSFEATURE, bytearray(b"\x00" + bytes(data)))

    def get_feature(self):
        buf = bytearray(REPORT_LEN + 1)
        self._fcntl.ioctl(self._fd, HIDIOCGFEATURE, buf, True)
        return _strip_report_id(buf)

    def close(self):
        os.close(self._fd)


def open_transport(path=None):
    if sys.platform.startswith("linux"):
        return HidrawTransport(path)
    return HidapiTransport(path)
