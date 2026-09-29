# vrox.vpn

VPN-клиент на базе hysteria2, работающий строго в TUN-режиме (без
SOCKS5/HTTP-прокси). Приложение на Tauri (Rust + React) — Linux и
iOS/iPadOS; на Mac (Apple Silicon) ставится та же iOS-сборка из
TestFlight. Подробности — ниже и в [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

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

## iPhone, iPad и Mac

VPN-тоннель реализован через `NetworkExtension` (`NEPacketTunnelProvider`)
— без привилегированного sidecar-процесса и без `pf`/`nftables`.
Распространяется через **TestFlight** (внутреннее тестирование, не
публичный App Store). Одна и та же iOS-сборка ставится на iPhone, iPad и
Mac с Apple Silicon; отдельной macOS-сборки нет. Обновления приходят
через сам TestFlight.

## Структура репозитория

- `app/` — Tauri-приложение (Rust backend + React frontend), общее для
  всех платформ; iOS-проект — `app/src-tauri/gen/apple/`.
- `ios/` — `NEPacketTunnelProvider`-расширение (Swift, `TunnelExtension/`)
  и скрипты: `build-go-framework.sh` (Go → `.xcframework`),
  `build-testflight.sh` (сборка `.ipa` для App Store Connect).
- `packaging/hysteria2-patch/` — форк `apernet/hysteria` с патчем
  directDomains и Go-пакетом `netunnel` (байт-слайс адаптация ядра
  hysteria2 для встраивания в NE-расширение через `gomobile bind`).
- `docs/ARCHITECTURE.md` — подробная архитектурная документация:
  privileged-слой на Linux, NetworkExtension, найденные и исправленные
  баги, причины архитектурных решений.

## Сборка из исходников

Linux — обычный Tauri-цикл:

```bash
cd app && pnpm install && pnpm tauri build
```

iOS (iPhone, iPad, Mac) — `.ipa` для TestFlight одной командой (Go-
фреймворк → архив Tauri → экспорт с ручной подписью; требует сертификат
Apple Distribution и App Store provisioning-профили, см. doc-комментарий
в начале скрипта). Загрузка — через Transporter или `xcrun altool`:

```bash
./ios/build-testflight.sh
```
