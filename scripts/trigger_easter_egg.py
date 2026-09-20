#!/usr/bin/env python3
"""
Trigger the Freaky 67 easter egg animation on the OPad display.

Cross-platform support: works seamlessly on Windows and Linux,
whether opad-daemon is currently running or not.
"""

import sys
import os
import time
import struct
import json
import socket
import subprocess

try:
    import serial
    import serial.tools.list_ports
except ImportError:
    # On Windows, try using espressif python env if system python lacks pyserial
    if sys.platform == "win32":
        idf_py = r"C:\Users\paella\.espressif\python_env\idf5.5_py3.14_env\Scripts\python.exe"
        if os.path.exists(idf_py) and sys.executable != idf_py:
            ret = subprocess.run([idf_py, __file__] + sys.argv[1:])
            sys.exit(ret.returncode)

    print("Error: pyserial is required. Run: pip install pyserial", file=sys.stderr)
    sys.exit(1)


def get_windows_pipe_path():
    try:
        out = subprocess.check_output(
            ["powershell", "-NoProfile", "-Command", "[System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value"],
            text=True
        )
        sid = out.strip()
        return rf"\\.\pipe\opad-ipc-{sid}" if sid else ""
    except Exception:
        return ""


def get_linux_socket_path():
    xdg = os.environ.get("XDG_RUNTIME_DIR")
    if xdg:
        p = os.path.join(xdg, "opad", "daemon.sock")
        if os.path.exists(p):
            return p
    uid = os.getuid() if hasattr(os, "getuid") else 1000
    p = f"/tmp/opad-{uid}/daemon.sock"
    if os.path.exists(p):
        return p
    if xdg:
        return os.path.join(xdg, "opad", "daemon.sock")
    return f"/tmp/opad-{uid}/daemon.sock"


class DaemonConnection:
    """Cross-platform IPC client: Named Pipe on Windows, Unix domain socket on Linux."""

    def __init__(self):
        self.pipe = None
        self.sock = None

    def connect(self):
        if sys.platform == "win32":
            pipe_path = get_windows_pipe_path()
            if not pipe_path:
                raise RuntimeError("Could not determine Windows user SID")
            self.pipe = open(pipe_path, "r+b", buffering=0)
        else:
            sock_path = get_linux_socket_path()
            self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            self.sock.settimeout(2.0)
            self.sock.connect(sock_path)

    def send_msg(self, obj):
        data = json.dumps(obj).encode("utf-8")
        header = struct.pack("<I", len(data))
        if self.pipe:
            self.pipe.write(header + data)
        else:
            self.sock.sendall(header + data)

    def recv_msg(self):
        if self.pipe:
            raw_len = self.pipe.read(4)
            if len(raw_len) < 4:
                raise EOFError("Connection closed")
            length = struct.unpack("<I", raw_len)[0]
            raw_data = self.pipe.read(length)
            if len(raw_data) < length:
                raise EOFError("Incomplete data received")
            return json.loads(raw_data.decode("utf-8"))
        else:
            raw_len = self._recv_exact(4)
            length = struct.unpack("<I", raw_len)[0]
            raw_data = self._recv_exact(length)
            return json.loads(raw_data.decode("utf-8"))

    def _recv_exact(self, n):
        buf = bytearray()
        while len(buf) < n:
            chunk = self.sock.recv(n - len(buf))
            if not chunk:
                raise EOFError("Socket closed")
            buf.extend(chunk)
        return bytes(buf)

    def close(self):
        if self.pipe:
            try:
                self.pipe.close()
            except Exception:
                pass
            self.pipe = None
        if self.sock:
            try:
                self.sock.close()
            except Exception:
                pass
            self.sock = None

    def __enter__(self):
        self.connect()
        return self

    def __exit__(self, *args):
        self.close()


def find_com_port():
    for p in serial.tools.list_ports.comports():
        # Match ESP32-S3 or OPad device (VID 0x303A)
        if p.vid == 0x303A:
            return p.device

    if sys.platform == "win32":
        return "COM3"
    else:
        for dev in ["/dev/ttyACM0", "/dev/ttyACM1", "/dev/ttyUSB0"]:
            if os.path.exists(dev):
                return dev
        return "/dev/ttyACM0"


def send_serial_easter_egg(port):
    s = serial.Serial(port, 115200, timeout=1)
    time.sleep(0.1)
    s.write(b"FREAKY67\n")
    s.flush()
    time.sleep(0.3)
    s.close()


def trigger_via_daemon():
    # Attempt 1: Try native TriggerEasterEgg on daemon with updated IPC
    try:
        with DaemonConnection() as conn:
            conn.send_msg({"Handshake": {"client_version": "1.0.0-rc", "client_protocol": 1}})
            _ = conn.recv_msg()
            conn.send_msg("TriggerEasterEgg")
            resp = conn.recv_msg()
            if resp == "EasterEggTriggered":
                return
    except Exception:
        pass

    # Attempt 2: Fallback for running daemon without TriggerEasterEgg IPC
    with DaemonConnection() as conn:
        conn.send_msg({"Handshake": {"client_version": "1.0.0-rc", "client_protocol": 1}})
        _ = conn.recv_msg()
        conn.send_msg("PrepareFlash")
        resp = conn.recv_msg()
        port = resp.get("ReadyForFlash", {}).get("port") or find_com_port()
        send_serial_easter_egg(port)
        conn.send_msg("FinishFlash")
        _ = conn.recv_msg()


def trigger_direct():
    port = find_com_port()
    send_serial_easter_egg(port)


def main():
    success = False
    try:
        trigger_via_daemon()
        success = True
    except Exception:
        pass

    if not success:
        try:
            trigger_direct()
            success = True
        except Exception as e:
            print(f"Failed to connect to device: {e}", file=sys.stderr)
            sys.exit(1)

    print("[OPad] Freaky 67 easter egg triggered successfully on device!")


if __name__ == "__main__":
    main()
