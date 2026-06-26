# vrox.vpn

VPN-клиент на базе hysteria2, работающий строго в TUN-режиме (без
SOCKS5/HTTP-прокси). Десктопное приложение на Tauri (Rust + React) —
поддерживает Linux и macOS, с разными механизмами доставки/обновления
на каждой платформе (см. ниже и [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)).

## Linux (Ubuntu)

```bash
wget -O /tmp/vrox.vpn.deb "https://github.com/beardrubyblue/VroxVPN/releases/latest/download/vrox.vpn_amd64.deb"
sudo apt install /tmp/vrox.vpn.deb
```

Если раньше была установлена старая версия (`vroxory-vpn`) — `apt` сам
её заменит, ничего удалять вручную не нужно.

После установки запускается ярлыком «vrox.vpn» в меню приложений.
Привилегированные операции (TUN-интерфейс, nftables kill switch) идут
через `pkexec` + `polkit`-правило, ставится автоматически при первом
запуске.

Обновления приложение проверяет само (`version.json` в этом репозитории)
и при наличии новой версии скачивает и ставит `.deb` через тот же
привилегированный helper — без отдельных действий пользователя.

## macOS

VPN-тоннель реализован через `NetworkExtension` (`NEPacketTunnelProvider`)
— без привилегированного sidecar-процесса и без `pf`/`nftables`. Подробно
архитектура описана в [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md),
раздел «macOS/NetworkExtension».

Распространяется через **TestFlight** (внутреннее тестирование, не
публичный App Store) — обновления приходят через сам TestFlight,
никакого отдельного механизма в приложении для macOS нет.

## Структура репозитория

- `app/` — Tauri-приложение (Rust backend + React frontend), общее для
  обеих платформ.
- `macos-ext/` — Xcode-проект `NEPacketTunnelProvider`-расширения для
  macOS (Swift) и скрипты сборки/упаковки (`build-release.sh` — для
  локального теста, `build-testflight.sh` — сборка для загрузки в
  App Store Connect).
- `packaging/hysteria2-patch/` — форк `apernet/hysteria` с патчем
  directDomains и Go-пакетом `netunnel` (байт-слайс адаптация ядра
  hysteria2 для встраивания в NE-расширение через `gomobile bind`).
- `docs/ARCHITECTURE.md` — подробная архитектурная документация:
  privileged-слой на Linux, миграция macOS на NetworkExtension, найденные
  и исправленные баги, причины архитектурных решений.

## Сборка из исходников

Linux — обычный Tauri-цикл:

```bash
cd app && pnpm install && pnpm tauri build
```

macOS — единая команда (Go-фреймворк → Xcode `.appex` → Tauri `.app` →
встраивание расширения → DMG для локального теста):

```bash
./macos-ext/build-release.sh
```

Для сборки артефакта под TestFlight/App Store Connect —
`./macos-ext/build-testflight.sh` (требует Apple Distribution + Mac
Installer Distribution сертификаты и App Store provisioning-профили,
см. doc-комментарий в начале скрипта).
