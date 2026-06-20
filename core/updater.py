"""Проверка и установка обновлений бинарника hysteria2 и самого приложения."""
import hashlib
import os
import subprocess
import urllib.request

import requests

from core.installer import download_hysteria2, get_binary_path, is_installed
from core.privileged import run_privileged

# hysteria2 ставится из нашего форка с пином на фиксированный тег (см.
# core/installer.py), а не с "latest" апстрима — поэтому "последняя версия"
# это просто та, что мы сами сейчас публикуем, без обращения к GitHub API.
# Строка должна совпадать с VERSION в packaging/hysteria2-patch/build.sh.
EXPECTED_VERSION = "v2.9.2-vroxory1"


class Updater:
    def get_latest_version(self) -> str:
        return EXPECTED_VERSION

    def get_installed_version(self) -> str | None:
        if not is_installed():
            return None

        try:
            result = subprocess.run(
                [get_binary_path(), "version"],
                capture_output=True,
                text=True,
                timeout=5,
            )
        except (OSError, subprocess.TimeoutExpired):
            return None

        output = result.stdout + result.stderr
        return EXPECTED_VERSION if EXPECTED_VERSION in output else None

    def check_update(self) -> dict:
        installed = self.get_installed_version()
        latest = self.get_latest_version()
        return {
            "installed": installed,
            "latest": latest,
            "update_available": installed != latest,
        }

    def update(self, progress_callback=None) -> bool:
        try:
            download_hysteria2(progress_callback)
        except (OSError, requests.RequestException, RuntimeError):
            return False
        return True


def _version_tuple(version: str) -> tuple:
    version = version.strip()
    if version.startswith("v"):
        version = version[1:]
    return tuple(int(x) for x in version.split("."))


class AppUpdater:
    CURRENT_VERSION = "3.0.0"
    VERSION_URL = "https://net.vroxory.com/vpn/version.json"
    # Fallback если основной сервер недоступен
    VERSION_URL_FALLBACK = "https://raw.githubusercontent.com/beardrubyblue/VroxVPN/main/version.json"

    UPDATE_DEB_PATH = "/tmp/vroxory-update.deb"

    def get_current_version(self) -> str:
        return self.CURRENT_VERSION

    def check_update(self, timeout: int = 5) -> dict | None:
        try:
            data = None
            for url in (self.VERSION_URL, self.VERSION_URL_FALLBACK):
                try:
                    resp = requests.get(url, timeout=timeout)
                    resp.raise_for_status()
                    data = resp.json()
                    print(f"[app-updater] получен version.json с {url}")
                    break
                except (requests.RequestException, ValueError) as exc:
                    print(f"[app-updater] {url} недоступен: {exc}")
                    continue

            if data is None:
                print("[app-updater] оба источника version.json недоступны, пропускаю")
                return None

            latest = data.get("version", "")
            try:
                update_available = _version_tuple(latest) > _version_tuple(self.CURRENT_VERSION)
            except ValueError:
                update_available = False

            return {
                "current": self.CURRENT_VERSION,
                "latest": latest,
                "update_available": update_available,
                "download_url": data.get("download_url", ""),
                "changelog": data.get("changelog", ""),
                "sha256": data.get("sha256", ""),
            }
        except Exception as exc:
            print(f"[app-updater] непредвиденная ошибка check_update: {exc}")
            return None

    def download_and_install(
        self, download_url: str, expected_sha256: str = "", progress_callback=None
    ) -> bool:
        tmp_path = self.UPDATE_DEB_PATH
        try:
            print(f"[app-updater] скачивание {download_url} -> {tmp_path}")

            def reporthook(block_num, block_size, total_size):
                if progress_callback and total_size > 0:
                    downloaded = min(block_num * block_size, total_size)
                    progress_callback(downloaded, total_size)

            urllib.request.urlretrieve(download_url, tmp_path, reporthook=reporthook)
            print("[app-updater] скачивание завершено")

            if expected_sha256:
                digest = hashlib.sha256()
                with open(tmp_path, "rb") as f:
                    for chunk in iter(lambda: f.read(8192), b""):
                        digest.update(chunk)
                if digest.hexdigest().lower() != expected_sha256.lower():
                    print("[app-updater] проверка sha256 не пройдена, установка отменена")
                    return False

            print("[app-updater] устанавливаю через apt-get")
            # apt-get install (а не dpkg -i) сам подтягивает недостающие
            # зависимости — без этого пришлось бы вручную запускать
            # apt-get install -f после dpkg. Идёт через privileged_helper.sh,
            # который проверяет, что путь равен ровно UPDATE_DEB_PATH.
            result = run_privileged(["apt-install", tmp_path])
            print(f"[app-updater] apt-get install -> код {result.returncode}")
            return result.returncode == 0
        except Exception as exc:
            print(f"[app-updater] ошибка установки обновления: {exc}")
            return False
        finally:
            if os.path.exists(tmp_path):
                os.remove(tmp_path)
                print(f"[app-updater] временный файл {tmp_path} удалён")
