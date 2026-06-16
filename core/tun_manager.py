"""Управление подключением hysteria2 в TUN-режиме."""
import os
import subprocess
import threading

from core import config_gen
from core.installer import get_binary_path
from core.dns_manager import DNSManager

CONNECTED_MARKERS = ("tun started", "client up and running", "connected")
TUN_IFACE = "tun-vroxory"


class TunManager:
    def __init__(self):
        self.process = None
        self._reader_thread = None
        self._connected = False
        self._stop_requested = False
        self._used_pkexec = False
        self._last_server = None

        self._auto_reconnect = True
        self._reconnect_attempts = 0
        self._max_reconnect_attempts = 5
        self._reconnect_delay = 3.0

        self.dns_manager = DNSManager()
        self.dns_protection_enabled = False

        self.on_connected = None
        self.on_disconnected = None
        self.on_error = None
        self.on_log = None
        self.on_reconnecting = None

    @property
    def is_connected(self) -> bool:
        return self._connected

    def connect(self, server: dict) -> None:
        self._stop_requested = False
        self._connected = False
        self._auto_reconnect = True
        self._used_pkexec = os.geteuid() != 0
        self._last_server = server

        config_path = config_gen.generate_config(server)
        binary = get_binary_path()

        self._loosen_rp_filter()

        if self._used_pkexec:
            cmd = ["pkexec", binary, "client", "--config", str(config_path)]
        else:
            cmd = [binary, "client", "--config", str(config_path)]

        try:
            self.process = subprocess.Popen(
                cmd,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                bufsize=1,
            )
        except OSError as exc:
            if self.on_error:
                self.on_error(str(exc))
            return

        self._reader_thread = threading.Thread(target=self._read_output, daemon=True)
        self._reader_thread.start()

    def _loosen_rp_filter(self) -> None:
        """Linux отбрасывает TUN-трафик строгим reverse-path filter — без
        этого пакеты к серверу маршрутизируются, но ответы дропаются ядром."""
        args = [
            "sysctl", "-w",
            "net.ipv4.conf.all.rp_filter=2",
            "net.ipv4.conf.default.rp_filter=2",
        ]
        cmd = ["pkexec"] + args if self._used_pkexec else args
        subprocess.run(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    def _read_output(self) -> None:
        if not self.process or not self.process.stdout:
            return

        for line in self.process.stdout:
            line = line.rstrip()
            if not line:
                continue
            if self.on_log:
                self.on_log(line)

            lowered = line.lower()
            if not self._connected and any(marker in lowered for marker in CONNECTED_MARKERS):
                self._connected = True
                self._reconnect_attempts = 0
                if self.on_connected:
                    self.on_connected()
                if self.dns_protection_enabled:
                    self.dns_manager.enable()

        exit_code = self.process.wait()
        was_connected = self._connected
        self._connected = False

        if self._stop_requested:
            if self.on_disconnected:
                self.on_disconnected()
            return

        if (
            exit_code != 0
            and self._auto_reconnect
            and self._reconnect_attempts < self._max_reconnect_attempts
        ):
            self._reconnect_attempts += 1
            delay = min(self._reconnect_delay * (2 ** (self._reconnect_attempts - 1)), 60)
            if self.on_reconnecting:
                self.on_reconnecting(self._reconnect_attempts, delay)
            timer = threading.Timer(delay, self._do_reconnect)
            timer.daemon = True
            timer.start()
            return

        if exit_code != 0 or not was_connected:
            if self.on_error:
                self.on_error(f"hysteria2 завершился с кодом {exit_code}")
        else:
            if self.on_disconnected:
                self.on_disconnected()

    def _do_reconnect(self) -> None:
        if self._last_server and not self._stop_requested:
            self.connect(self._last_server)

    def disconnect(self) -> None:
        self._stop_requested = True
        self._auto_reconnect = False
        self._reconnect_attempts = 0

        if self.dns_protection_enabled and self.dns_manager.is_active():
            self.dns_manager.disable()

        if self.process:
            self._signal_process("TERM")
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self._signal_process("KILL")
                self.process.wait()

        self._connected = False
        self._cleanup_interface()

    def _signal_process(self, signal_name: str) -> None:
        """Если hysteria2 запущен через pkexec, он работает с euid 0 — обычный
        os.kill() от непривилегированного процесса вернёт EPERM."""
        if self._used_pkexec:
            subprocess.run(
                ["pkexec", "kill", f"-{signal_name}", str(self.process.pid)],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
        elif signal_name == "TERM":
            self.process.terminate()
        else:
            self.process.kill()

    def _cleanup_interface(self) -> None:
        cmd = ["ip", "link", "delete", TUN_IFACE]
        if self._used_pkexec:
            cmd = ["pkexec"] + cmd
        subprocess.run(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
