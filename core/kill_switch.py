"""Kill switch на nftables — блокирует трафик мимо TUN-интерфейса."""
import os
import subprocess

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


class KillSwitch:
    def enable(self, tun_interface: str = "tun-vroxory", vpn_server_ip: str = "") -> bool:
        rules = NFT_RULES_TEMPLATE.format(
            table=TABLE_NAME,
            tun_interface=tun_interface,
            vpn_server_ip=vpn_server_ip or "0.0.0.0/32",
        )
        cmd = ["nft", "-f", "-"]
        if os.geteuid() != 0:
            cmd = ["pkexec"] + cmd

        result = subprocess.run(cmd, input=rules, text=True, capture_output=True)
        return result.returncode == 0

    def disable(self) -> bool:
        cmd = ["nft", "delete", "table", "inet", TABLE_NAME]
        if os.geteuid() != 0:
            cmd = ["pkexec"] + cmd

        result = subprocess.run(cmd, capture_output=True, text=True)
        return result.returncode == 0

    def is_active(self) -> bool:
        result = subprocess.run(
            ["nft", "list", "table", "inet", TABLE_NAME],
            capture_output=True,
            text=True,
        )
        return result.returncode == 0
