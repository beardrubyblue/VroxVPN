"""Kill switch на nftables — блокирует трафик мимо TUN-интерфейса."""
import ipaddress
import socket
import subprocess

from core.privileged import run_privileged

TABLE_NAME = "vroxory_killswitch"

NFT_RULES_TEMPLATE = """\
table inet {table} {{
    chain output {{
        type filter hook output priority 0; policy drop;
        oifname "lo" accept
        oifname "{tun_interface}" accept
        ip daddr {vpn_server_ip} accept
        ip daddr 192.168.0.0/16 accept
        ip daddr 10.0.0.0/8 accept
        ip daddr 172.16.0.0/12 accept
    }}
}}
"""


def _safe_server_ip(host: str) -> str:
    """Возвращает host только если это валидный IPv4-литерал (резолвя
    имя при необходимости) — никакая непроверенная строка из подписки не
    должна попадать напрямую в текст nft-правил, выполняемых как root."""
    if not host:
        return "0.0.0.0/32"
    try:
        ipaddress.IPv4Address(host)
        return host
    except ValueError:
        pass
    try:
        resolved = socket.gethostbyname(host)
        ipaddress.IPv4Address(resolved)
        return resolved
    except (socket.gaierror, ValueError, OSError):
        return "0.0.0.0/32"


class KillSwitch:
    def enable(self, tun_interface: str = "tun-vroxory", vpn_server_ip: str = "") -> bool:
        rules = NFT_RULES_TEMPLATE.format(
            table=TABLE_NAME,
            tun_interface=tun_interface,
            vpn_server_ip=_safe_server_ip(vpn_server_ip),
        )
        result = run_privileged(["nft-apply"], input_data=rules)
        return result.returncode == 0

    def disable(self) -> bool:
        result = run_privileged(["nft-delete-table", TABLE_NAME])
        return result.returncode == 0

    def is_active(self) -> bool:
        result = subprocess.run(
            ["nft", "list", "table", "inet", TABLE_NAME],
            capture_output=True,
            text=True,
        )
        return result.returncode == 0
