# vrox.vpn

VPN-клиент для Linux Ubuntu на базе hysteria2, работающий строго в
TUN-режиме (без SOCKS5/HTTP-прокси).

## Установка

```bash
wget -O /tmp/vrox.vpn.deb "https://github.com/beardrubyblue/VroxVPN/releases/latest/download/vrox.vpn_amd64.deb"
sudo apt install /tmp/vrox.vpn.deb
```

Если раньше была установлена старая версия (`vroxory-vpn`) — `apt` сам
её заменит, ничего удалять вручную не нужно.

После установки запускается ярлыком «vrox.vpn» в меню приложений.
Обновления приложение проверяет само и предложит установить новую
версию через баннер в настройках.
