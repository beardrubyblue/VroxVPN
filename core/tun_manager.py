"""Управление подключением hysteria2 в TUN-режиме."""
import os
import subprocess
import threading

from core import config_gen
from core.installer import get_binary_path
from core.dns_manager import DNSManager
from core.privileged import run_privileged

CONNECTED_MARKERS = ("tun started", "client up and running", "tun listening")
TUN_IFACE = "tun-vroxory"


class TunManager:
    def __init__(self):
        self.process = None
        self._reader_thread = None
        self._connected = False
        self._stop_requested = False
        self._used_pkexec = False
        self._last_server = None
        self._config_path = None

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
        self._config_path = config_path
        binary = get_binary_path()

        self._loosen_rp_filter()
        # если предыдущий запуск завершился аварийно, интерфейс tun-vroxory
        # может остаться висеть в ядре — тогда hysteria2 падает с
        # "device or resource busy". Чистим заранее (без проверки ошибок).
        self._cleanup_interface()

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
        run_privileged(["loosen-rp-filter"])

    def _read_output(self) -> None:
        if not self.process or not self.process.stdout:
            return

        had_fatal = False
        last_fatal_message = ""

        for line in self.process.stdout:
            line = line.rstrip()
            if not line:
                continue
            if self.on_log:
                self.on_log(line)

            lowered = line.lower()

            if "fatal" in lowered:
                # hysteria2 иногда логирует "TUN listening" ДО фактического
                # открытия устройства и лишь потом падает с FATAL — поэтому
                # success-маркер мог сработать чуть раньше ошибки. Раз FATAL
                # пришёл, попытку считаем неудачной независимо от этого.
                had_fatal = True
                last_fatal_message = line

            if not self._connected and any(marker in lowered for marker in CONNECTED_MARKERS):
                self._connected = True
                if self.on_connected:
                    self.on_connected()
                if self.dns_protection_enabled:
                    self.dns_manager.enable()

        exit_code = self.process.wait()
        was_connected = self._connected and not had_fatal
        self._connected = False

        if was_connected:
            # сбрасываем счётчик попыток только когда уверены, что сессия
            # была рабочей — иначе ложный success-маркер ("connected to
            # server" / "TUN listening" перед крахом) держит счётчик на 0
            # и реконнект повторяется бесконечно с одинаковой задержкой.
            self._reconnect_attempts = 0

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
            message = last_fatal_message or f"hysteria2 завершился с кодом {exit_code}"
            if self.on_error:
                self.on_error(message)
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
        os.kill() от непривилегированного процесса вернёт EPERM.

        self.process.pid — pid pkexec-обёртки (supervisor), а НЕ настоящего
        root-процесса hysteria2: pkexec форкает целевую программу отдельным
        процессом и сам остаётся жить как монитор. Сигнал, отправленный
        Popen.pid, убивал только supervisor — сам hysteria2 оставался висеть
        и держать TUN-устройство захваченным, из-за чего следующее
        подключение падало с "device or resource busy". Поэтому ищем
        настоящий процесс по уникальному пути конфига (см. privileged_helper.sh
        kill-hysteria), а не по pid."""
        if self._used_pkexec:
            run_privileged(["kill-hysteria", signal_name, str(self._config_path)])
        elif signal_name == "TERM":
            self.process.terminate()
        else:
            self.process.kill()

    def _cleanup_interface(self) -> None:
        run_privileged(["delete-tun", TUN_IFACE])
