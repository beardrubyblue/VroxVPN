# Шлюз vroxgw

`vroxgw` — vrox.vpn без интерфейса для Linux-машины в локальной сети
(например VM на Synology). Он поднимает тоннель hysteria2 по подписке и
пропускает через него трафик клиентов, которых роутер отправил на эту
машину. Как это устроено, описано в `docs/ARCHITECTURE.md`, раздел
«Шлюз vroxgw».

## Сборка пакета (на Маке)

```bash
./packaging/hysteria2-patch/build.sh   # vroxcore для linux-amd64
./packaging/vroxgw/build-deb.sh        # → packaging/vroxgw/build/vroxgw_<версия>_amd64.deb
```

Нужны Go и Docker.

## Требования к VM

- **От 2 vCPU, лучше 3–4.** Тоннель целиком работает в userspace
  (vroxcore): на ~350 Мбит/с он занимает больше двух ядер. На 1 vCPU
  скорость упиралась в процессор и не превышала ~90 Мбит/с.
- Сетевая карта VM — VirtIO (`ethtool -i ens3` → `virtio_net`), не e1000.
- Пакет сам поднимает UDP-буферы сокетов до 7.5 МБ
  (`/etc/sysctl.d/60-vroxgw.conf`): с дефолтными 208 КБ скорость
  разгонялась и обваливалась из-за потерь на приёме.

## Установка на VM (Ubuntu / Debian x86_64)

```bash
scp packaging/vroxgw/build/vroxgw_*_amd64.deb user@192.168.1.2:
ssh user@192.168.1.2
sudo apt install ./vroxgw_*_amd64.deb
sudo nano /etc/vroxgw/config.toml      # subscription_url, при необходимости lan
sudo systemctl start vroxgw
vroxgw status
```

У VM должен быть **статический IP**. Сеть VM в Synology VMM — в режиме
моста (bridge), чтобы машина была в той же LAN, что и клиенты. Если на
VM включён `ufw`, разрешите форвардинг (`DEFAULT_FORWARD_POLICY="ACCEPT"`
в `/etc/default/ufw`), иначе он отбросит транзитный трафик.

## Команды

| Команда | Что делает |
|---|---|
| `vroxgw status` | статус тоннеля, узел, пинг, последняя ошибка |
| `sudo vroxgw nodes` | узлы из подписки (● — текущий) |
| `sudo vroxgw switch <имя>` | закрепить узел; `auto` — снова лучший по пингу |
| `journalctl -u vroxgw -f` | лог демона и vroxcore |

## Kill switch

При `killswitch = true` (по умолчанию) шлюз пересылает трафик клиентов,
только пока тоннель работает. Если тоннель упал или сервис остановлен,
у клиентов нет интернета, но и утечки мимо VPN нет. При
`killswitch = false` во время сбоя тоннеля трафик идёт напрямую, а при
остановке сервиса шлюз перестаёт пересылать трафик совсем.

## Памятка для администратора роутера (MikroTik)

Шлюз: `192.168.1.2`. Ему нужно, чтобы выбранные клиенты отправляли
интернет-трафик на этот адрес, а DNS-запросы шли наружу, а не на роутер:
иначе DNS пойдёт мимо VPN.

**Вариант A — шлюз через DHCP (проще).** Клиенты сами ходят на VM, путь
симметричный:

```
/ip dhcp-server option add name=gw-vpn code=3 value="'192.168.1.2'"
/ip dhcp-server option add name=dns-vpn code=6 value="'1.1.1.1'"
/ip dhcp-server option sets add name=vpn options=gw-vpn,dns-vpn
/ip dhcp-server lease set [find mac-address=AA:BB:CC:DD:EE:FF] dhcp-option-set=vpn
```

Lease клиента должен быть статическим. Клиенту нужно переподключиться,
чтобы получить новые опции.

**Вариант B — policy routing (RouterOS 7).** Клиенты ничего не замечают,
роутер сам отправляет их трафик на VM:

```
/routing table add name=vpn fib
/ip route add dst-address=0.0.0.0/0 gateway=192.168.1.2 routing-table=vpn check-gateway=ping
/ip firewall address-list add list=via-vpn address=192.168.1.50
/ip firewall mangle add chain=prerouting src-address-list=via-vpn \
    dst-address=!192.168.1.0/24 action=mark-routing new-routing-mark=vpn passthrough=no
```

В варианте B трафик к VM и обратно проходит через роутер по-разному:
туда через роутер, обратно от VM прямо к клиенту, потому что они в одной
подсети. Шлюз маскирует трафик, и это снимает основную часть проблемы. Но
fasttrack для `via-vpn` лучше выключить. `check-gateway=ping` при
недоступной VM вернёт клиентов на прямой выход, а это обходит kill switch
шлюза. Самый чистый вариант — вынести VM и VPN-клиентов в отдельный VLAN.

## Проверка

- На VM: `vroxgw status` показывает «подключён», а `curl ifconfig.me`
  возвращает IP VPN-сервера.
- На клиенте из списка: тот же IP, а российский сайт (например
  `2ip.ru`) показывает домашний IP. Это значит, что RU-обход работает.
- `sudo systemctl stop vroxgw`: у клиента пропадает интернет (kill
  switch). После `start` интернет возвращается.
