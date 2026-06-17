# vrox.vpn

VPN-клиент для Linux Ubuntu на базе hysteria2, работающий строго в
TUN-режиме (без SOCKS5/HTTP-прокси).

## Установка

```bash
wget -O /tmp/vroxory-vpn.deb "https://github.com/beardrubyblue/VroxVPN/releases/latest/download/vroxory-vpn_<версия>_amd64.deb"
sudo apt install /tmp/vroxory-vpn.deb
```

Актуальную версию смотри на странице
[релизов](https://github.com/beardrubyblue/VroxVPN/releases/latest).

После установки запускается командой `vroxory-vpn` или ярлыком «vrox.vpn»
в меню приложений. Обновления приложение проверяет само и предложит
установить новую версию через баннер.
