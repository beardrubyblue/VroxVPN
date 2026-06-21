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

## macOS (ветка `macos-support`) — подготовлено на Linux, НЕ проверено

Реальной macOS-машины на момент написания этого раздела не было — всё
ниже собиралось и компилировалось только под Linux-таргет (cross-check
под `x86_64-apple-darwin` невозможен без rustup: нет std для этого
таргета в дистрибутивном `rustc`, и часть API чисто macOS-специфичная).
**Перед тем как доверять этому в проде — пройти весь чеклист ниже на
самом Mac.**

### Что сделано

- `engine.rs` разбит на общий диспетчер (`Slot`/`EngineState`/
  `ActiveConnection` — общие типы) + `engine/linux.rs` (весь прежний
  Linux-код, поведение не менялось, проверено `cargo build` — Linux-
  сборка не пострадала) + `engine/macos.rs` (новый, см. ниже). Публичный
  API (`spawn_client`, `kill_client`, `enable_killswitch`,
  `disable_killswitch`, `ensure_polkit_rule`, `loosen_rp_filter`,
  `cleanup_interface`, `cleanup_orphans`) одинаковый на обеих платформах
  — `commands.rs`/`lib.rs` не знают, на чём они работают.
- `engine/macos.rs`: вместо pkexec/polkit — `osascript -e 'do shell
  script "..." with administrator privileges'` (см. `elevated_shell_command`,
  экранирование строится через `shell_quote`+`applescript_quote`, есть
  unit-тесты на экранирование). **Важно**: в отличие от polkit-правила,
  это НЕ даёт гарантии "пароль один раз навсегда" — macOS кеширует
  авторизацию ненадолго (порядка нескольких минут), не бессрочно.
- `resources/privileged_helper_macos.sh` — аналог `privileged_helper.sh`
  с тем же контрактом подкоманд, где это применимо (`kill-hysteria`,
  `is-running`, `kill-all-hysteria` — `pkill`/`pgrep` работают на macOS
  так же, как на Linux), плюс `pf-apply`/`pf-restore` вместо
  `nft-apply`/`nft-delete-table`.
- Kill switch на macOS спроектирован **иначе**, чем на Linux: nftables-
  вариант разрешает только TUN-интерфейс по имени (`tun-vroxory`,
  фиксированное имя, которое мы сами задаём). На macOS имя
  utun-интерфейса **назначает ядро** (utun0, utun1, ...) — мы не можем
  знать его заранее так же, как на Linux. Поэтому pf-ruleset блокирует
  исходящий трафик на физических интерфейсах (enumerated через
  `ifconfig -l` в самом shell-скрипте) кроме как до самого VPN-сервера/
  приватных диапазонов, оставляя TUN вообще не упомянутым (не нужно
  знать его имя, если мы не блокируем его, а блокируем остальные).
- `tauri.conf.json` → `bundle.macOS`: `minimumSystemVersion: "12.0"`,
  `hardenedRuntime: true`, `entitlements: "macos/entitlements.plist"`,
  `signingIdentity: null` (заполнить на Mac — Developer ID Application
  идентификатор сертификата, либо передать через `APPLE_SIGNING_IDENTITY`
  при сборке).
- `macos/entitlements.plist` — минимальный набор под hardened runtime
  + WKWebView (allow-jit, allow-unsigned-executable-memory,
  disable-library-validation). Без App Sandbox — распространение
  напрямую через `.dmg` (Developer ID + notarization), не через Mac
  App Store.
- `packaging/hysteria2-patch/build.sh` — добавлены таргеты
  `darwin:amd64`/`darwin:arm64` (Go cross-compile с Linux обычно
  работает без CGO, но TUN-код форка может на это полагаться — не
  проверено, что darwin-бинарник из этого скрипта реально собирается и
  работает).

### Чеклист на самом Mac (по порядку)

1. Установить Xcode + command line tools, Rust (`rustup`), Node/pnpm.
2. Собрать `vroxcore` под darwin: либо `packaging/hysteria2-patch/
   build.sh` (кросс с Linux, не проверено), либо напрямую на Mac
   `GOOS=darwin GOARCH=arm64 go build ...` в каталоге форка. Результат
   переименовать/положить в `app/src-tauri/binaries/` как
   `vroxcore-x86_64-apple-darwin` / `vroxcore-aarch64-apple-darwin`
   (именно так, с префиксом `vroxcore-`, а не `hysteria2-vroxory-...`,
   как называет их сам build.sh для GitHub-релиза форка — см. константы
   `SIDECAR_NAME_X86`/`SIDECAR_NAME_ARM` в `engine/macos.rs`).
3. `pnpm tauri dev` — первая проверка, что приложение хотя бы
   запускается на macOS (отдельно от VPN-функциональности).
4. Проверить `osascript`-эскалацию вручную (`elevated_shell_command`
   строит `do shell script ... with administrator privileges`) —
   убедиться, что промпт появляется, путь к бинарнику/конфигу с
   пробелами (например, внутри `.app`-бандла) корректно экранируется.
5. **pf-ruleset — самое неопределённое место.** Проверить вживую:
   - что `ifconfig -l` в `pf-apply` реально находит физический
     интерфейс (Wi-Fi обычно `en0`, может отличаться);
   - что `pfctl -f -` с нашим ruleset'ом реально применяется и блокирует
     трафик мимо VPN (проверить вручную: выключить TUN, попытаться
     достучаться куда-то кроме сервера/приватных сетей — должно
     блокироваться);
   - что `pf-restore` корректно восстанавливает сохранённый ruleset
     (`pfctl -sr` до применения), а не просто гасит pf целиком, если у
     пользователя был свой firewall до нас.
6. Решить вопрос с повторными паролями (`osascript` спрашивает чаще,
   чем хотелось бы) — рекомендуемый путь: privileged helper через
   `SMAppService` (регистрируется один раз через System Settings,
   дальше работает как daemon без повторных промптов) вместо
   `osascript` на каждый вызов. Это отдельная, более крупная задача
   (нужен XPC-протокол между приложением и helper, отдельный
   подписанный бинарник в `Contents/Library/LaunchServices/`) — не
   делалась, только спроектирован fallback на `osascript`.
7. Codesign + notarization: `signingIdentity` в `tauri.conf.json`
   (Developer ID Application), затем
   `xcrun notarytool submit ... --apple-id ... --team-id ... --wait`
   (Tauri CLI умеет это автоматизировать через переменные окружения
   `APPLE_ID`/`APPLE_PASSWORD`/`APPLE_TEAM_ID` или API-ключ — см.
   `pnpm tauri build` docs для macOS).
8. После первой успешной сборки — обновить `version.json`/GitHub
   Release по той же схеме, что и Linux-релиз (см. `README.md`).

### iOS — отдельный проект, не просто "ещё одна платформа"

Ничего из текущего privileged-слоя (`pkexec`/`osascript`/`pf`/процесс-
sidecar) на iOS не применимо вообще: там нет привилегированных
процессов и нет sidecar-бинарников для обычных приложений. VPN на iOS
делается только через `NEPacketTunnelProvider` — отдельный Xcode-таргет
(network extension), который запускается в своём процессе с урезанными
правами и жёстким лимитом памяти, требует entitlement
`com.apple.developer.networking.networkextension` (выдаётся Apple по
запросу, не автоматически), и hysteria2 туда обычно встраивают как
статическую библиотеку (`gomobile`/cgo → `.xcframework`), а не как
внешний процесс. Это отдельная задача проектирования, не входит в этот
macOS-чеклист — планировать отдельно, когда дойдёт очередь.

## macOS → NetworkExtension — решение принято, переход начат

Живое тестирование sidecar+osascript+pf-подхода (см. чеклист выше)
показало: архитектурно работает, но упирается в TCC (`osascript ...
with administrator privileges` не может выполнить файл из ~/Documents/
~/Desktop/iCloud Drive — обойдено стейджингом в `/tmp`, см.
`engine/macos.rs::stage_helper_outside_tcc`), нет "пароль один раз
навсегда" как на Linux, и pf-killswitch остаётся неподтверждённым на
живом трафике. Решение (принято в разговоре с пользователем): не
чинить эти болевые точки точечно, а перейти на `NEPacketTunnelProvider`
— тот же механизм, который и так обязателен для iOS (см. раздел выше),
общая инвестиция для обеих платформ. Полный план (фазы 0-7, оценка
трудоёмкости, открытые вопросы) — `hazy-jumping-owl.md`, живёт локально
на машине, где его сделали (не часть репозитория, личный план-файл).

Статус по фазам плана:

- **Фаза 0 (спайк) — пройдена, на реальном Mac.** `gomobile bind
  -target macos` собрал `.xcframework` из тестового пакета с
  byte-slice сигнатурами (`[]byte` → `NSData*`, ошибки → `NSError**`)
  поверх `core/client` форка — главный архитектурный риск (жизнеспособность
  gomobile-границы) подтверждён. Замер памяти: ~24.4MB RSS для
  client+QUIC+TLS стека без directDomains (он на NE-пути не нужен, см.
  Фазу 3 ниже) — есть запас под лимиты NE-процесса.
- **Фаза 1 (вынос Go-логики) — начата, на Linux.** Новый пакет
  `packaging/hysteria2-patch/netunnel/` (копируется в `app/internal/
  netunnel/` тем же `build.sh`, что и `directmatch`/`dnssniff`):
  - `virtual_tun.go` — `virtualTun`, реализация интерфейса `tun.Tun` из
    `apernet/sing-tun` (`io.ReadWriter` + `N.VectorisedWriter` + `Close`)
    БЕЗ настоящего файлового дескриптора — пакеты ходят через каналы.
    Подтверждено чтением исходников sing-tun: `tun.NewSystem` (gVisor-
    стек) работает только через интерфейс `Tun`, не привязан к тому,
    как тот получает байты — `tun.New()` (создание настоящего
    устройства) можно не вызывать вообще.
  - `handler.go` — `relayHandler`, релей-логика (`HyClient.TCP`/`UDP` +
    `io.Copy`) скопирована из `app/internal/tun/server.go::tunHandler`
    (не импортирована — тот тип unexported в своём пакете). directDomains
    сюда НЕ перенесён осознанно.
  - `netunnel.go` — `StartTunnel(configJSON)`/`WritePacket`/`ReadPacket`/
    `Stop` — методы только на `[]byte`/строках (требование gomobile bind).
    `Config` — минимальный JSON (server/auth/sni/insecure), БЕЗ obfs/
    QUIC-тюнинга из `config_gen.rs` — полный паритет полей не сделан,
    сначала проверялась сама архитектура.
  - Проверено: `go build`/`go vet` чисто, причём весь `build.sh`
    прогнан с нуля (свежий клон апстрима) — компилируется на реальных
    типах `sing-tun`/`hysteria/core/client`, не на угаданных сигнатурах.
  - ⚠ НЕ проверено: `gomobile bind` именно этого пакета (нужен Xcode),
    реальный packet round-trip через `NEPacketTunnelFlow`, throughput/
    GC-нагрузка при реальной скорости пакетов.
- **Фаза 2+ (Xcode-таргет, Swift `PacketTunnelProvider`, codesign,
  entitlement) — не начаты**, требуют реального Mac/Xcode/Apple Developer
  аккаунта, ведутся отдельной сессией там же.

Деление работы: Go/Rust-сторона (Фазы 1, 3 — `excludedRoutes` вместо
DNS-сниффинга, 5 — JSON-конфиг вместо YAML, 7 — toggle `connection_backend`
в `settings.rs`) — на любой машине с Go/Rust, без Xcode. Xcode/Swift/
codesign/entitlement-сторона (Фазы 0 остаток, 2, 6) — только на Mac.
