"""Защита DNS — переключение системного резолвера на доверенные серверы."""
import os
from pathlib import Path

from core.privileged import run_privileged

RESOLV_CONF = "/etc/resolv.conf"

PROTECTED_DNS = ["1.1.1.1", "1.0.0.1", "8.8.8.8"]

PROTECTED_RESOLV_CONTENT = (
    "nameserver 1.1.1.1\n"
    "nameserver 1.0.0.1\n"
    "nameserver 8.8.8.8\n"
    "options edns0 trust-ad\n"
)

DEFAULT_RESOLV_CONTENT = "nameserver 8.8.8.8\n"


class DNSManager:
    backup_path = Path("/tmp/vroxory-vpn/resolv.conf.backup")

    def enable(self) -> bool:
        self.backup_path.parent.mkdir(parents=True, exist_ok=True)
        self._save_backup()

        # /etc/resolv.conf обычно симлинк на /run/systemd/resolve/stub-resolv.conf,
        # который systemd-resolved постоянно перегенерирует — писать "через" симлинк
        # бессмысленно, изменения затираются почти сразу. Снимаем симлинк и кладём
        # обычный файл, который резолвер не трогает. Содержимое идёт через stdin
        # helper-скрипта — без shell-интерполяции, экранирование не требуется.
        result = run_privileged(["write-resolv-conf"], input_data=PROTECTED_RESOLV_CONTENT)
        success = result.returncode == 0
        print(f"[dns] enable() -> {success}")
        return success

    def disable(self) -> bool:
        if self.backup_path.exists():
            raw = self.backup_path.read_text()
        else:
            raw = f"FILE:{DEFAULT_RESOLV_CONTENT}"

        kind, _, payload = raw.partition(":")

        if kind == "SYMLINK":
            result = run_privileged(["relink-resolv-conf", payload])
        else:
            result = run_privileged(["write-resolv-conf"], input_data=payload)

        success = result.returncode == 0
        if success:
            self.backup_path.unlink(missing_ok=True)
        print(f"[dns] disable() -> {success}")
        return success

    def _save_backup(self) -> None:
        # на каталоге /tmp/vroxory-vpn у пользователя rwx, поэтому unlink чужого
        # файла разрешён даже если предыдущий запуск создал его через pkexec/root
        self.backup_path.unlink(missing_ok=True)

        if os.path.islink(RESOLV_CONF):
            target = os.readlink(RESOLV_CONF)
            self.backup_path.write_text(f"SYMLINK:{target}")
            return

        try:
            content = Path(RESOLV_CONF).read_text()
        except OSError:
            content = DEFAULT_RESOLV_CONTENT
        self.backup_path.write_text(f"FILE:{content}")

    def check_leak(self) -> dict:
        try:
            content = Path(RESOLV_CONF).read_text()
        except OSError:
            return {"protected": False, "nameservers": []}

        nameservers = []
        for line in content.splitlines():
            line = line.strip()
            if not line.startswith("nameserver"):
                continue
            parts = line.split()
            if len(parts) >= 2:
                nameservers.append(parts[1])

        protected = bool(nameservers) and all(ns in PROTECTED_DNS for ns in nameservers)
        return {"protected": protected, "nameservers": nameservers}

    def is_active(self) -> bool:
        return self.backup_path.exists()
