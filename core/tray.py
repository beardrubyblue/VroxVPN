"""Иконка в системном трее.

Запускается как отдельный GTK3-процесс (core/tray_process.py), поскольку
PyGObject не позволяет смешивать GTK3 (нужен для AyatanaAppIndicator3,
без которого GNOME Shell не показывает трей-иконки) и GTK4 (главное окно)
в одном процессе. Общение — через простой построчный протокол по
stdin/stdout дочернего процесса.
"""
import subprocess
import sys
import threading
from pathlib import Path

TRAY_SCRIPT = Path(__file__).resolve().parent / "tray_process.py"


class TrayIcon:
    def __init__(self):
        self.on_show = None
        self.on_toggle = None
        self.on_quit = None
        self.on_select_server = None

        self._process = None
        self._reader_thread = None

    def start(self) -> None:
        try:
            self._process = subprocess.Popen(
                [sys.executable, str(TRAY_SCRIPT)],
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.DEVNULL,
                text=True,
                bufsize=1,
            )
        except OSError as exc:
            print(f"[tray] не удалось запустить процесс трея: {exc}")
            self._process = None
            return

        self._reader_thread = threading.Thread(target=self._read_output, daemon=True)
        self._reader_thread.start()

    def _read_output(self) -> None:
        if not self._process or not self._process.stdout:
            return
        for raw in self._process.stdout:
            line = raw.strip()
            if line:
                self._handle_line(line)

    def _handle_line(self, line: str) -> None:
        if line == "SHOW":
            if self.on_show:
                self.on_show()
        elif line == "TOGGLE":
            if self.on_toggle:
                self.on_toggle()
        elif line == "QUIT":
            if self.on_quit:
                self.on_quit()
        elif line.startswith("SELECT:"):
            name = line[len("SELECT:"):]
            if self.on_select_server:
                self.on_select_server(name)

    def _send(self, line: str) -> None:
        if not self._process or not self._process.stdin:
            return
        try:
            self._process.stdin.write(line + "\n")
            self._process.stdin.flush()
        except (BrokenPipeError, OSError):
            pass

    def stop(self) -> None:
        if not self._process:
            return
        self._send("QUIT_PROCESS")
        try:
            self._process.wait(timeout=3)
        except subprocess.TimeoutExpired:
            self._process.kill()

    def update_status(self, connected: bool, server_name: str = "") -> None:
        self._send(f"STATUS:{1 if connected else 0}:{server_name}")

    def update_servers(self, servers: list, selected_name: str = "") -> None:
        names = "\x1f".join(server["name"] for server in servers)
        self._send(f"SERVERS:{names}")
        if selected_name:
            self._send(f"SELECTED:{selected_name}")
