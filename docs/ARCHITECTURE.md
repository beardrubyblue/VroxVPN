# Архитектура переезда на Tauri (ветка tauri-rewrite)

Цель: один UI-кодбейс (Tauri 2 = Rust-backend + веб-фронтенд) на Linux,
Windows, macOS, Android, iOS вместо текущего GTK4/libadwaita-приложения
(оно остаётся в ветке `main`, не трогаем).

## Три слоя

1. **vroxcore (Go)** — протокольное ядро hysteria2 + TUN-обработка +
   directDomains-bypass. Источник: наш форк apernet/hysteria,
   `packaging/hysteria2-patch/` в ветке `main` (build.sh, bump.sh,
   direct-domains.patch, dnssniff.go, directmatch.go).

   Сейчас bypass на Linux сделан через `AF_PACKET`-сниффинг DNS на
   физическом интерфейсе + `SO_BINDTODEVICE` для прямого dial — оба
   механизма линуксовые и не компилируются на других ОС. До переноса на
   Windows/macOS/мобильные платформы нужно **один раз** переделать этот
   кусок в портируемом виде: матчинг домена прямо по пакетам, которые
   идут через TUN-fd (движок и так их видит), плюс платформенная точка
   подмены «исключить сокет из VPN»:
   - Linux: `SO_BINDTODEVICE` (как сейчас)
   - Windows: `IP_UNICAST_IF`
   - macOS: `IP_BOUND_IF`
   - Android: `VpnService.protect(socket)`
   - iOS: точку исключения предоставляет `NEPacketTunnelProvider`

2. **Платформенная интеграция** — как vroxcore встроен в ОС:
   - **Linux/Windows/macOS** — vroxcore как отдельный процесс,
     Tauri-термин "sidecar". Rust-backend (`app/src-tauri/src/engine.rs`)
     спавнит его и управляет — то же самое, что сейчас делает
     `core/tun_manager.py` в Python-версии. Привилегии — pkexec/UAC/
     osascript (переписать `core/privileged.py` на Rust), kill switch —
     nftables/WFP/pfctl соответственно.
   - **Android** — vroxcore собирается через `gomobile` в `.aar`,
     оборачивается Kotlin-классом `VpnService` (кастомный нативный
     плагин, не из коробки Tauri) — он владеет TUN-fd и кормит его в
     vroxcore. Kill switch/DNS — через `VpnService.Builder`.
   - **iOS** — vroxcore через gomobile/cgo в `.xcframework`, оборачивается
     Swift-кодом внутри `NEPacketTunnelProvider` — это отдельный
     Xcode-таргет (extension-процесс), webview Tauri туда физически не
     достаёт, никакого UI там нет. Жёсткий лимит памяти extension-процесса
     — нужно проверять, что vroxcore туда вписывается. Требует
     entitlement `com.apple.developer.networking.networkextension`.

3. **Tauri-shell (Rust + веб-фронтенд)** — общий UI для главного окна на
   всех платформах: статус, список серверов, настройки (ru-bypass,
   автозапуск, kill switch). Дёргает Rust-команды (`app/src-tauri/src/
   commands.rs`) через `invoke()` — это только control-plane
   (старт/стоп/статус), не сам трафик.

## Что НЕ становится общим из-за выбора Tauri

Kill switch, DNS-защита, эскалация привилегий и мобильная VPN-обвязка
(VpnService/NetworkExtension) платформо-специфичны независимо от UI-
фреймворка — это требование самой ОС. Общий UI закрывает только слой 3.

## Порядок платформ

Linux (этот скаффолд) → Windows → Android (первая платформа с настоящим
нативным VPN-плагином) → macOS → iOS (сложнее и менее обкатано — у Tauri
нет известного прецедента VPN-клиента на mobile, в отличие от Flutter/
Hiddify).

## Текущий статус скаффолда

- `app/` — создан через `create-tauri-app` (React + TypeScript + pnpm),
  productName/identifier/version подогнаны под проект.
- Rust (`cargo`/`rustc` 1.93.1) и зависимости Tauri для Linux
  (`webkit2gtk`, `librsvg2`, `libsoup-3.0`, `libayatana-appindicator3`)
  установлены через apt. `pnpm tauri dev` собирается и открывает окно.
- **Sidecar подтверждён рабочим end-to-end**: бинарник vroxcore (наш
  форк hysteria2, скопирован из `packaging/hysteria2-patch/build/` как
  `app/src-tauri/binaries/vroxcore-x86_64-unknown-linux-gnu`,
  зарегистрирован в `tauri.conf.json` → `bundle.externalBin` и в
  `capabilities/default.json` → `shell:allow-execute`) реально
  запускается из Rust через `tauri-plugin-shell` и возвращает вывод во
  фронтенд. Команда `engine_version` (`commands.rs`) и кнопка
  «Проверить версию ядра» в `App.tsx` — рабочий пример этого пути.
  Бинарник НЕ закоммичен (добавлен в `.gitignore`) — для повторной
  сборки взять из GitHub Release `hysteria2-fork-v2.9.2-1` или пересобрать
  через `packaging/hysteria2-patch/build.sh`.
- **`connect`/`disconnect` подтверждены рабочими end-to-end на реальном
  сервере.** `engine.rs` копирует контракт `core/tun_manager.py`:
  `loosen-rp-filter` + `delete-tun` (best-effort, до спавна) через
  `pkexec privileged_helper.sh`, сам клиент — `pkexec <vroxcore> client
  --config <path>` через `app.shell().command(...)`, `disconnect` —
  `kill-hysteria TERM <config>` + `delete-tun` (kill по пути конфига,
  не по pid — pkexec отдаёт pid своей обёртки, а не настоящего
  root-процесса, см. комментарий в `kill-hysteria` в самом
  `privileged_helper.sh`). `EngineState` (Tauri managed state) хранит
  `CommandChild` + путь конфига под `Mutex`.
  Тестировалось вручную через кнопку в `App.tsx` с конфигом, уже
  сгенерированным старым Python-приложением (путь передаётся как
  параметр `connect`, как и задумано архитектурой) — реальное TCP/UDP
  через TUN, DNS-сниффер и directDomains (1773 домена geosite) все
  поднялись и проотключились чисто.
- **Важный найденный нюанс**: оба приложения (Python и Tauri) сейчас
  жёстко используют одно и то же имя интерфейса `tun-vroxory`. Если
  одновременно держать включёнными старое и новое приложение — второе
  ломает TUN-fd первого через `delete-tun` (поймали как "FATAL read
  tun: file descriptor in bad state" / "not pollable"). Это не баг
  Tauri-обвязки, а ожидаемое следствие двух VPN-клиентов на одно имя
  интерфейса — для реального параллельного запуска (например, тестов)
  потребуется развести имена интерфейсов.
- **Подписка и генерация конфига портированы и подтверждены рабочими.**
  `subscription.rs` — порт `core/subscription.py` (парсинг
  `hysteria2://` URI, base64-фоллбек, QUIC-параметры из `fm`,
  `Subscription-Userinfo`). `config_gen.rs` — порт `core/config_gen.py`
  (YAML для TUN-режима, резолв IP сервера для exclude-маршрутов,
  obfs/salamander, QUIC-секция). `connect` теперь принимает целиком
  `Server` (а не путь к файлу) и сам генерирует конфиг через
  `config_gen::generate_config`. Фронтенд: поле URL подписки → «Получить
  серверы» (`fetch_servers`) → выбор из `<select>` → «Подключиться».
  Проверено end-to-end на реальной подписке: список серверов, YAML
  совпадает по структуре с питоновским (включая QUIC-тюнинг и
  obfs/salamander), TUN поднимается и чисто гасится.
- **Geoip/geosite (ru_bypass) портированы.** `geoip.rs`/`geosite.rs` —
  порт `core/geoip.py`/`core/geosite.py`: встроенный снимок + обновление
  в `~/.config/vroxory-vpn/geoip|geosite` (тот же путь, что у Python-
  приложения). `config_gen::generate_config` принимает `ru_bypass: bool`
  и добавляет geoip-исключения в `ipv4Exclude`/`ipv6Exclude` +
  `directDomains` из geosite. Фронтенд: галочка + кнопки "Обновить
  geoip/geosite". Проверено на реальном подключении: 8627 IPv4-
  диапазонов, 1773 домена, `TUN direct domains enabled` + `DNS sniffer`
  в логах движка.
- **Настройки приложения портированы.** `settings.rs` — порт
  `core/settings.py`, читает/пишет ТОТ ЖЕ `~/.config/vroxory-vpn/
  settings.json`, что и Python-приложение — merge поверх дефолтов
  сохраняет чужие ключи (`kill_switch_enabled`, `autostart_enabled` и
  т.п.) нетронутыми. Фронтенд подгружает URL подписки/последний сервер/
  ru_bypass при старте через `useEffect`, сохраняет при изменениях.
  Проверено: перезапуск приложения восстанавливает всё без ручного
  ввода.
- **Резолвинг путей к ресурсам исправлен.** `resources.rs` —
  `app.path().resolve(_, BaseDirectory::Resource)` вместо
  `CARGO_MANIFEST_DIR` для `privileged_helper.sh` и встроенных снимков
  geoip/geosite (`engine.rs`, `geoip.rs`, `geosite.rs` теперь принимают
  `&AppHandle`). Работает одинаково в dev и в собранном приложении —
  относительные пути совпадают с `tauri.conf.json` → `bundle.resources`.
  Sidecar-бинарник (`vroxcore`) — отдельная конвенция (не "ресурс"):
  `engine::sidecar_binary_path()` сначала ищет рядом с текущим
  исполняемым файлом (так Tauri размещает `externalBin` в собранном
  приложении), иначе — в `src-tauri/binaries/` (dev-режим). Нужен
  путь, а не сразу запуск, поэтому `ShellExt::sidecar()` здесь не
  подходит — он сам спавнит процесс, а нам нужно сначала обернуть его
  в `pkexec`.

## Следующие шаги (не сделаны)

1. Windows — новый слой платформенной интеграции (sidecar остаётся, но
   привилегии через UAC вместо pkexec, kill switch через WFP вместо
   nftables, DNS через `netsh` вместо resolv.conf). `engine.rs` в
   текущем виде целиком про Linux, для Windows будет отдельный модуль,
   не обобщение этого.
2. Resource-резолвинг захардкожен на Linux-триплет
   (`vroxcore-x86_64-unknown-linux-gnu` в `engine.rs`) — при портировании
   на Windows/arm64 нужно либо прокинуть target triple через `build.rs`
   (`cargo:rustc-env=TARGET=...`), либо завести отдельные константы
   по платформе.
