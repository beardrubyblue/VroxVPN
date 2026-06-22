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

## macOS (ветка `macos-support`) — sidecar-путь опробован и удалён

Первая версия порта (sidecar-процесс + `osascript`-эскалация + pf
killswitch, аналог Linux-модели) была написана, затем реально собрана и
проверена на живом Mac — и сразу показала архитектурные болячки (TCC
блокирует привилегированный скрипт из ~/Documents, нет "пароль один раз
навсегда" как у polkit, pf-killswitch не подтверждён на живом трафике).
Решение: не чинить это точечно, а перейти на NetworkExtension сразу
(он и так обязателен для iOS) — см. раздел ниже. Sidecar-реализация
(`engine/macos.rs` в старом виде, `resources/privileged_helper_macos.sh`,
`pf-apply`/`pf-restore`, `osascript`-эскалация) удалена из кода целиком,
не оставлена под флагом "на всякий случай" — два параллельных backend'а
на одну платформу, из которых поедет только один, не имеет смысла
поддерживать. Если детали той реализации нужны для справки — смотреть
git-историю `engine/macos.rs` до коммита, убирающего sidecar.

Из той попытки осталось то, что не привязано к sidecar-модели и нужно
независимо от архитектуры VPN-слоя:
- `tauri.conf.json` → `bundle.macOS`: `minimumSystemVersion: "12.0"`,
  `hardenedRuntime: true`, `entitlements: "macos/entitlements.plist"`,
  `signingIdentity: null` (заполнить на Mac, либо через переменную
  окружения `APPLE_SIGNING_IDENTITY` при сборке).
- `macos/entitlements.plist` — `allow-jit`/`allow-unsigned-executable-
  memory` (требования Tauri/WebKit под hardened runtime) +
  `com.apple.developer.networking.networkextension` (нужен для NE,
  выдаётся Apple по отдельному запросу — статус запроса см. в проекте
  у того, кто ведёт Mac-сессию). `disable-library-validation` убран —
  был нужен только sidecar-бинарнику.
- Codesign + notarization (`xcrun notarytool submit ... --wait`, либо
  через `pnpm tauri build` с `APPLE_ID`/`APPLE_PASSWORD`/`APPLE_TEAM_ID`)
  — общий шаг, не зависящий от sidecar/NE.

### iOS и macOS теперь делят одну архитектуру — не "ещё одна платформа"

На iOS привилегированных процессов и sidecar-бинарников для обычных
приложений не бывает вообще — VPN там можно сделать только через
`NEPacketTunnelProvider`. Раньше в этом документе iOS планировался как
отдельная задача "на потом", после отдельного macOS-пути. С переходом
macOS на NetworkExtension это уже не два проекта, а один — см. раздел
ниже про конкретный статус по фазам.

## macOS → NetworkExtension — решение принято, переход начат

Живое тестирование sidecar+osascript+pf-подхода (удалён из кода, см.
раздел выше) показало: архитектурно работает, но упирается в TCC
(`osascript ... with administrator privileges` не может выполнить файл
из ~/Documents/~/Desktop/iCloud Drive — был обойдён стейджингом в /tmp),
нет "пароль один раз навсегда" как на Linux, и pf-killswitch остаётся
неподтверждённым на живом трафике. Решение (принято в разговоре с
пользователем): не
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
  - **Проверено на реальном Mac: `gomobile bind -target macos` И
    `-target ios`** дают полный `.xcframework` с заголовком, где
    `StartTunnel`/`TunnelHandle.readPacket/writePacket/stop` видны Swift
    ровно как задумано (`[]byte` → `NSData*`, ошибки → `NSError**`).
    При этом нашёлся и исправлен реальный баг: путь `app/internal/
    netunnel` не собирался через `gomobile bind` — Go запрещает импорт
    `internal/`-пакетов кодом снаружи дерева модуля-владельца, а
    `gomobile bind` генерирует свою wrapper-package во ВРЕМЕННОМ отдельном
    модуле, то есть снаружи. Фикс: `build.sh` теперь копирует в
    `app/netunnel` (не `app/internal/netunnel`) — сам netunnel всё ещё
    может импортировать `app/internal/utils` (он остаётся в дереве
    модуля `app/v2`), просто сам не помечен `internal`. Экспортированные
    `Config`/`ObfsConfig`/`BandwidthConfig`/`CongestionConfig` гомобайл
    забиндил как Obj-C классы с предупреждениями "skipped field" для
    вложенных структур/`uint32` — не блокирует ничего, потому что
    реальный публичный API работает через JSON-строку
    (`NetunnelStartTunnel(configJSON, &error)`), а не через прямую
    конструкцию `Config` из Swift.
  - ⚠ Ещё не проверено: реальный packet round-trip через
    `NEPacketTunnelFlow` (нужен Xcode-таргет, см. Фазу 2 ниже),
    throughput/GC-нагрузка при реальной скорости пакетов.
- **Фаза 2 — пройдена на реальном Mac, end-to-end до уровня реального
  сетевого пакета.** Новая структура `macos-ext/` (не часть Tauri-сборки,
  отдельный Xcode workspace):
  - `project.yml` + `xcodegen` — два таргета: `VroxVPNHost` (голый
    тест-харнесс, NSWindow с кнопками Connect/Disconnect через
    `NETunnelProviderManager`, НЕ продакшен-UI) и `VroxTunnelExtension`
    (настоящий `.appex`, `NEPacketTunnelProvider`).
  - `build-go-framework.sh` — прогоняет `packaging/hysteria2-patch/
    build.sh` (тот же клон форка) и `gomobile bind -target macos` поверх
    `app/netunnel`, кладёт `.xcframework` в `macos-ext/Frameworks/`
    (build-артефакт, не коммитится).
  - `PacketTunnelProvider.swift` — `startTunnel` зовёт
    `NetunnelStartTunnel`, поднимает `NEPacketTunnelNetworkSettings`,
    дальше два цикла: `packetFlow.readPackets` → `TunnelHandle.
    writePacket` (входящие пакеты в Go) и фоновый поток с блокирующим
    `TunnelHandle.readPacket` → `packetFlow.writePackets` (исходящие из
    Go). `killswitch` (`includeAllNetworks`) — свойство
    `NETunnelProviderProtocol` в хосте, не `NEPacketTunnelNetworkSettings`
    в расширении (это разные объекты, спутать легко).

  **Реально собрано, подписано настоящим Developer ID
  (`com.apple.developer.networking.networkextension` entitlement уже
  выдан Apple) и запущено через `NETunnelProviderManager` на живом Mac.**
  Лог системы (`nesessionmanager`/`neagent`) подтвердил: профиль VPN
  регистрируется, ОС запускает `.appex`, `utun`-интерфейс реально
  поднимается с IPv4/IPv6/DNS-настройками от `setTunnelNetworkSettings`,
  Go-код реально пытается открыть UDP-сокет и отправить пакет на
  сконфигурированный сервер. Дальше упирается только в тестовый
  placeholder-конфиг (`127.0.0.1:1`/недостижимый порт) — то есть весь
  путь Swift→Go→gVisor→sandbox подтверждён, не догадка.

  Попутно найдены и исправлены три реальные проблемы (не очевидны
  заранее, фиксирую для будущих похожих задач):
  1. **`@main`/`@NSApplicationMain` не сработали** на этой связке Xcode/
     Swift для голого `NSApplicationDelegate`-класса без storyboard
     (`applicationDidFinishLaunching` ни разу не вызывался — подтверждено
     файловым лог-выводом, не предположение). Фикс — явный `main.swift`:
     `NSApplication.shared.delegate = AppDelegate(); NSApplicationMain(...)`.
     Это в свою очередь конфликтует с Xcode-фичей "debug dylib" (быстрая
     инкрементальная пересборка через alias `_main` →
     `__debug_main_executable_dylib_entry_point`) — пришлось добавить
     `ENABLE_DEBUG_DYLIB: NO` в `project.yml`. Касается только
     тест-харнесса (`VroxVPNHost`), не самого расширения.
  2. **Свободные C-функции gomobile не мостятся в Swift `throws`.**
     Автомост `NSError**` → `throws` у Swift-импортёра Objective-C
     работает только для ObjC-методов класса (`TunnelHandle.writePacket/
     readPacket/stop` — методы, там `try` работает нормально), но НЕ для
     `FOUNDATION_EXPORT`-функций верхнего уровня (`NetunnelStartTunnel`) —
     для них нужен явный `NSErrorPointer`. Имя Swift-модуля для `import`
     — `GoNetunnel` (из имени `.xcframework`), не `Netunnel` (имя
     Go-пакета) — видно из `module.modulemap` внутри фреймворка.
  3. **App Sandbox NE-расширения требует ДВА network-entitlement'а, не
     один.** `com.apple.security.network.client` покрывает только
     `network-outbound` (исходящий `connect`) — для UDP-сокета этого не
     хватает, потому что `net.ListenUDP` делает `bind()` на локальный
     эфемерный порт даже в чисто клиентском режиме, а это отдельная
     категория `network-bind`, требующая `com.apple.security.network.
     server`. Без него — `kernel(Sandbox) deny(1) network-bind
     local:*:0`, ошибка не из Go/gomobile слоя, а из ядра, видна только
     через `log show` с фильтром по `sandboxd`/`kernel`, в логе самого
     расширения — просто "operation not permitted" без объяснения, какое
     именно действие запрещено.

  IPv4/IPv6-определение протокола в `packetFlow.writePackets` исправлено
  сразу (читает верхний нибл первого байта пакета, 4 или 6) — изначально
  было захардкожено на `AF_INET`, что молча ломало бы весь IPv6-трафик.

  ⚠ Не проверено (нужен реальный hysteria2-сервер + время понаблюдать
  под нагрузкой): throughput/GC-нагрузка при реальной скорости пакетов,
  поведение `includeAllNetworks` при принудительном падении расширения
  (fail-open vs fail-closed — открытый вопрос Фазы 4 плана).
- **Фаза 1 продолжена — паритет конфига.** `netunnel.Config` теперь
  покрывает sni/insecure/pinSHA256/obfs(salamander)/bandwidth/congestion
  — то же подмножество, что `config_gen.rs` строит для YAML. Сознательно
  НЕ перенесено: quic-тюнинг (`Server.quic` в subscription.rs — поля вида
  `maxIdleTimeout`, ожидающие строку "30s"; `encoding/json` не парсит
  `time.Duration` из строк так же просто, как mapstructure/viper в
  YAML-пути — не стал угадывать) и gecko-obfs (экспериментален в самом
  upstream hysteria2).
- **Фаза 3 (`excludedRoutes` вместо DNS-сниффинга) — начата, но план
  скорректирован после проверки чисел.** Исходная идея (резолвить домены
  из `geosite::get_ru_domains()` в IP заранее, при генерации конфига) не
  учитывала, что там ~1736 доменов — массовый блокирующий DNS-резолв
  такого объёма на каждый connect означал бы реальные секунды задержки и
  кучу таймаутов (часть записей geosite — суффиксы, не сами резолвящиеся
  имена). Сделано: `config_gen::generate_excluded_routes()` — только
  статическая часть (IP сервера + приватные диапазоны + RU-geoip CIDR),
  без доменов. Домены из RU-bypass (для CDN, которые под geoip не
  попадают) — пока БЕЗ решения: вероятный путь — не сниффинг на отдельном
  физическом интерфейсе (как `dnssniff_linux.go`), а наблюдение прямо в
  `relayHandler.NewPacketConnection` самого `netunnel` (под NE DNS-трафик
  идёт через наш relay, NE не вырезает порт 53 из тоннеля так, как делает
  sing-tun AutoRoute на Linux) — но чтобы реально исключить разрешённый
  IP из тоннеля ПОСЛЕ старта, нужен Go→Swift callback (повторный вызов
  `setTunnelNetworkSettings` с обновлённым `excludedRoutes`), которого
  нет и не спроектирован — гадать про него с одной стороны (Go, без
  реального NE) смысла не было.
- **Фаза 5 (JSON-конфиг вместо YAML-на-диске) — начата.**
  `config_gen::generate_provider_config_json()` строит то же подмножество
  полей, что `generate_config()` для YAML, в виде `serde_json::Value` (не
  пишет на диск — под NE конфиг уходит в `NETunnelProviderProtocol.
  providerConfiguration` в памяти). Есть unit-тест, сверяющий имена полей
  буква-в-букву с json-тегами `netunnel.Config` — несовпадение здесь не
  поймает ни одна из сторон по отдельности, раз Go и Rust собираются и
  тестируются независимо.
- Обе новые функции (`generate_excluded_routes`, `generate_provider_config_json`)
  помечены `#[allow(dead_code)]` — вызывающего кода пока нет, потребитель
  это будущий control-bridge в `engine/macos.rs` (см. ниже).

Деление работы: Go/Rust-сторона (Фазы 1, 3 частично, 5) — на любой
машине с Go/Rust, без Xcode. Xcode/Swift/codesign/entitlement-сторона
(Фазы 0 остаток, 2, 6) — только на Mac. **Без `connection_backend`-
тоггла** — см. ниже, почему.

### Sidecar-модель удалена сразу, не оставлена под флагом

Когда решили перейти на NE, первый порыв был оставить старый sidecar-
путь живым под настройкой `connection_backend: legacy | network_extension`
"на всякий случай" (вдруг entitlement от Apple не дадут или затянут).
Отклонено: это ровно то бессмысленное наслоение, которого нужно
избегать — два backend'а на одну платформу, поддерживать оба вечно,
хотя ехать будет только один. Старый код удалён немедленно (см. раздел
"sidecar-путь опробован и удалён" выше), без флага.

### Абстракция `ActiveConnection`/`ConnectionHandle` (engine.rs)

При удалении sidecar-кода на macOS нашлась реальная утечка абстракции:
`ActiveConnection.child` был жёстко типизирован как `CommandChild`
(хэндл процесса) — общий для Linux и (старого) macOS, потому что оба
были sidecar-моделью. Под NE никакого процесса, который мы сами
породили, не существует вообще (тоннель живёт в `.appex`, управляемом
ОС) — поле `child: CommandChild` для этого случая физически не имеет
смысла.

Исправлено: `engine::ConnectionHandle` — platform-specific type alias
(`CommandChild` на Linux, `()` на macOS), `ActiveConnection.handle:
ConnectionHandle` вместо `.child: CommandChild`. `commands.rs` теперь
`let _ = conn.handle` (не `drop(conn.handle)` — `()` на macOS это
`Copy`-тип, явный `drop()` на нём триггерит компиляторский warning,
`let _ =` работает идентично на обеих платформах без warning'а) —
работает одинаково для обоих типов, не зная, что конкретно держит.
Проверено `cargo check`/`cargo build`/`cargo test` на реальном Mac (см.
раздел про control-bridge ниже) — все типы реально проверены компилятором,
не угадывались.

### Control-bridge к NEVPNManager — реализован, Rust напрямую, без Swift-CLI

Открытый вопрос выше ("когда появится рабочий NE-мост, обобщить
connect_inner") закрыт. **Готовый крейт `objc2-network-extension`
существует** (генерируется из заголовков NetworkExtension.framework,
не пришлось писать ручные `extern_class!`/`msg_send!` биндинги, как
предполагалось на момент написания плана) — добавлен как
target-specific зависимость (`[target.'cfg(target_os = "macos")'.
dependencies]` в `Cargo.toml`, вместе с `objc2`/`objc2-foundation`/
`block2`).

`engine/macos.rs` теперь реально реализует `spawn_client`/`kill_client`
через `NETunnelProviderManager`/`NETunnelProviderProtocol`/
`NEVPNConnection`:
- `load_or_create_manager_blocking` — `loadAllFromPreferencesWithCompletionHandler`,
  берёт `firstObject` или создаёт новый (конфигураций нашего приложения
  всегда максимум одна).
- `spawn_client` строит `NETunnelProviderProtocol` (providerBundleIdentifier
  = `com.vroxory.vpn.tunnel`, должен совпадать с `PRODUCT_BUNDLE_IDENTIFIER`
  в `macos-ext/project.yml`; `includeAllNetworks = true` — killswitch,
  см. Фазу 4), `providerConfiguration` = словарь с `configJSON` (из
  `config_gen::generate_provider_config_json`) + `ipv4Exclude`/
  `ipv6Exclude` (из `config_gen::generate_excluded_routes`, который для
  этого выведен из `#[allow(dead_code)]` в `#[cfg(target_os = "macos")]`
  — реальный потребитель появился), сохраняет (`saveToPreferences`),
  перезагружает (`loadFromPreferences`) и стартует
  (`connection().startVPNTunnelAndReturnError()`).
- `kill_client` грузит manager заново по bundle ID и зовёт
  `connection().stopVPNTunnel()` — не держит сквозную ссылку на manager
  между connect/disconnect, оба раза перезагружает из system preferences
  (надёжнее при перезапуске приложения между connect/disconnect).

**Реальная архитектурная проблема, найденная и решённая:**
`Retained<NETunnelProviderManager>` и `block2::RcBlock` — НЕ `Send`, а
Tauri требует `Send`-future от `async fn` команд
(`#[tauri::command] async fn connect`). Попытка использовать
`tokio::sync::oneshot` + `.await` напрямую внутри `async fn` с этими
типами на стеке между await-точками не компилируется (`future cannot be
sent between threads safely`). Решение: вся objc2-логика синхронна
(`std::sync::mpsc` вместо `tokio::sync::oneshot`, обычный блокирующий
`recv()` вместо `.await`) и целиком выполняется на одном потоке через
`tauri::async_runtime::spawn_blocking` — снаружи `spawn_client` остаётся
`async fn` (для паритета сигнатуры с Linux), но await пересекает только
`JoinHandle<Result<...>>`, чей результат — `Send`-типы (`()`/`String`),
сами objc2-объекты никогда не пересекают границу await. NE
completion-блоки вызываются на главном потоке (run loop приложения) —
блокирующий поток просто ждёт на `recv()`, не мешая GUI.

**Заодно закрыт *другой* открытый вопрос** ("connect_inner всегда
сначала пишет YAML-файл, под NE так не получится"): общая сигнатура
`engine::spawn_client` изменена с `(app, config_path: &str)` на
`(app, server: &Server, ru_bypass: bool) -> Result<(ConnectionHandle,
String), String>` — каждая платформа теперь сама решает, писать файл на
диск (Linux: `config_gen::generate_config` переехал ИЗ `commands.rs`
ВНУТРЬ `engine::linux::spawn_client`, поведение Linux не изменилось,
просто генерация конфига теперь на один уровень глубже) или строить
JSON в памяти (macOS). `kill_client`-сигнатуру трогать не понадобилось
— она и так была достаточно общей (строка — на Linux путь к файлу, на
macOS просто игнорируется).

Проверено на реальном Mac: `cargo build`/`cargo test` чисто (0
warnings), `pnpm tauri dev` запускается и не падает с новым кодом в
`engine/macos.rs` на старте (`cleanup_orphans`/`ensure_polkit_rule` —
no-op, как и раньше, просто теперь через реально подключённый модуль, а
не функции-заглушки).

⚠ Не проверено живым подключением к настоящему hysteria2-серверу через
сам Tauri-UI (только через тест-харнесс `macos-ext/VroxVPNHost`, см.
Фазу 2) — следующий логичный шаг проверки, не архитектурный риск.

### Живой тест через настоящий Tauri-UI — найден и исправлен критический баг с includeAllNetworks

Сделано: `.appex` встроен в реальный `vrox.vpn.app` (собран через `pnpm
tauri build --debug`), подписан вручную через `macos-ext/embed-into-
tauri-app.sh` (Tauri сам не подписывает с реальным entitlement —
`signingIdentity: null` даёт ad-hoc подпись без entitlements вообще,
скрипт переподписывает целиком тем же Developer-сертификатом и
провижининг-профилем, которые уже получил Xcode при сборке
`macos-ext`). По пути всплыли и решены:

- **AMFI убивал процесс при запуске**: `kernel: ... Hardened Runtime
  relaxation entitlements disallowed on System Extensions`. Apple
  запрещает сочетать `com.apple.developer.networking.networkextension`
  с любым hardened-runtime relaxation entitlement
  (`allow-jit`/`allow-unsigned-executable-memory`/
  `disable-library-validation`) на одном бинарнике. Убраны оба
  relaxation-entitlement'а из `macos/entitlements.plist` главного
  приложения — WKWebView продолжает работать нормально без них (JIT для
  JS выполняется в отдельном XPC-процессе `com.apple.WebKit.WebContent`
  со своими entitlements, не нашего бинарника это не касается;
  проверено: окно открывается, фронтенд рендерится).
- **Реальный коннект через настоящий UI — найден критический баг,
  потребовавший ДВУХ перезагрузок Mac, прежде чем причина стала ясна.**
  `includeAllNetworks = true` (killswitch, `engine/macos.rs::
  spawn_client_blocking` и тест-харнесс `AppDelegate.swift`) блокирует
  ВЕСЬ исходящий трафик системы сразу при переходе VPN-соединения в
  "Connecting" — ДО того, как `setTunnelNetworkSettings` вызван внутри
  расширения или `startTunnel` завершился успехом. `netunnel` сам
  устанавливает UDP-соединение к hysteria2-серверу ВНУТРИ `StartTunnel`
  (`singleUseConnFactory.New` → `net.ListenUDP`/`net.DialUDP`) — то есть
  собственный исходящий трафик тоннеля тоже попадает под этот же
  захват, и сам тоннель никогда не может подняться (chicken-and-egg).
  Внешне это выглядело как: приложение "подключается", весь интернет на
  Mac пропадает целиком (не только в приложении), обычное отключение
  через `scutil --nc stop`/System Settings не восстанавливало сеть.
  Подтверждено документацией/практикой Apple — известная, открыто
  обсуждаемая проблема, не баг этого кода: [Apple Developer Forums
  thread 677102](https://developer.apple.com/forums/thread/677102),
  [wireguard-apple mailing list о том же](https://www.mail-archive.com/wireguard@lists.zx2c4.com/msg06703.html).
  **Исправлено: `includeAllNetworks` убран целиком** — и из
  `engine/macos.rs`, и из тест-харнесса. Killswitch без него слабее
  (только `includedRoutes=[default]` в `NEPacketTunnelNetworkSettings`,
  работает только ПОСЛЕ удачного подключения, не защищает на время
  самого коннекта) — пересмотреть отдельно, когда remote relay
  подтверждён рабочим без этого флага; не делать это вслепую повторно.
- После фикса DNS-резолвинг хоста сервера также перенесён из `netunnel`
  (внутри песочницы расширения) в `config_gen::
  generate_provider_config_json` (Rust, обычный несэндбоксенный
  процесс) — отдельно от проблемы `includeAllNetworks`, но обнаружено в
  той же серии живых тестов: `net.ResolveUDPAddr` внутри App Sandbox
  расширения зависал на ~30с и проваливался ("no such host"). `server`
  в JSON-конфиге теперь резолвленный IP, не hostname; `sni` остаётся
  оригинальным именем (нужен для TLS, не для адреса сокета).

### Следующий живой тест — два новых бага, не связанных с includeAllNetworks

После фикса `includeAllNetworks` интернет на Mac больше не пропадал
целиком — главный риск закрыт. Но всплыли два отдельных бага:

- **Disconnect вешал ВСЁ приложение намертво** (не только кнопку), хотя
  VPN на уровне ОС реально отключался (`scutil --nc list` показывал
  Disconnected). Причина: `disconnect` — обычная синхронная
  Tauri-команда, вызывала `kill_client` прямо на своём потоке.
  `kill_client` внутри блокируется на `rx.recv()`, ожидая
  completion-callback от `loadAllFromPreferencesWithCompletionHandler`
  — а тот callback приходит через главный run loop. Если поток
  диспетчера команд Tauri и есть главный поток — он ждёт callback,
  который не может быть доставлен, потому что run loop на этом же
  потоке не крутится (тот же паттерн, который уже был решён для
  `connect` через `spawn_blocking`, просто не применён к `disconnect`).
  **Исправлено**: `disconnect`/`kill_client` стали `async` +
  `spawn_blocking` на обеих платформах (на Linux не дедлок, но всё
  равно лишнее удержание потока на ~3с блокирующего опроса —
  исправлено для единообразия сигнатур).
- **UI показывал "подключено" даже когда тоннель не поднялся.**
  `startVPNTunnelAndReturnError()` подтверждает только то, что ОС
  ПРИНЯЛА запрос на старт (переход в `.Connecting`) — не то, что
  `startTunnel` внутри расширения реально завершился успехом. Если
  `netunnel.StartTunnel` падает (например, хендшейк с сервером не
  проходит — см. ниже), `spawn_client` всё равно возвращал `Ok`, и
  Rust-сторона никогда не узнавала о реальном провале — состояние
  навсегда оставалось `Connected` в UI. На Linux аналогичный баг был
  закрыт раньше через проверку "процесс не умер в первые 1.5с"; на
  macOS процесса, который мы породили, не существует, поэтому источник
  правды — `NEVPNStatus`, не PID.
  **Исправлено**: `spawn_client` теперь регистрирует наблюдателя на
  `NEVPNStatusDidChangeNotification` через `NSNotificationCenter` и
  ждёт терминального статуса (`.Connected` — успех, `.Disconnected`/
  `.Invalid` — провал) с таймаутом 25с, прежде чем вернуть результат
  Tauri-команде. После успешного коннекта регистрируется второй,
  долгоживущий наблюдатель (`watch_for_unexpected_disconnect`) — если
  статус позже сам перейдёт в `Disconnected`/`Invalid`, пока состояние
  ещё `Connected`, эмитит `vpn-disconnected-unexpectedly`, как и на
  Linux (тот же контракт с фронтендом, источник события другой — NE
  статус, не выход процесса).

При этом самом тесте подтвердилось: хендшейк к `speed.vroxory.com`
по-прежнему не проходит с этой сети (`timeout: no recent network
activity`) — ICMP-пинг и UDP/443 reachability проходят нормально, но
сам QUIC-хендшейк нет. Та же проблема, что была зафиксирована в начале
работы над macOS-портом на sidecar-пути — внешняя, не связана с
переходом на NE. Не проверено: работает ли реальный коннект с другой
сети (мобильный хотспот) — следующий шаг, не часть этой сессии.

⚠ Дальше (после этих фиксов) ещё не проверено живым кликом —
осторожность после двух потерь интернета оправдала себя: каждый раунд
живого теста находил по новому реальному багу. Следующая попытка —
с этими тремя фиксами одновременно (includeAllNetworks убран, disconnect
не виснет, статус коннекта подтверждается реально).

### `tun.NewSystem` оказался не gVisor — найдено и переписано на настоящий gVisor

Со статусом "тоннель не поднялся" (после фиксов выше — честный провал,
не молчаливая ложь в UI) вскрылась причина: `netunnel.StartTunnel`
падал на `listen tcp4 100.100.100.101:0: bind: can't assign requested
address` — буквально на старте стека, не на хендшейке. Это вскрыло
фундаментальную, а не временную проблему: `tun.NewSystem` из форка
`apernet/sing-tun`, который весь Фазой 1 в коде и комментариях называли
"gVisor netstack" — на деле это **"System stack"**: он открывает
настоящий TCP-listener ОС, забинденный на IP виртуальной подсети, и
работает только если у настоящего TUN-устройства этот IP реально
назначен ядром (как на Linux/Windows sidecar-пути). У нашего
виртуального TUN под NE (просто Go-каналы, без файлового дескриптора)
такого IP в системе нет — бинд падает гарантированно, не из-за сети.
Настоящий gVisor (`tun.NewGVisor`) в этом форке `sing-tun` **вырезан
целиком** — там голая заглушка с ошибкой "gVisor is not supported in
this fork" (подтверждено чтением исходников и git-историей: в
оригинальном `sagernet/sing-tun` gVisor есть, в `apernet`-форке,
который тащит hysteria2, — нет, и версии разошлись слишком далеко
(v0.2.6 vs v0.8.x), чтобы просто подменить зависимость).

**Исправлено: подключили `gvisor.dev/gvisor` напрямую, без обёртки
sing-tun.** Тот же подход, что использует wireguard-go в своём
netstack-режиме:
- `virtual_tun.go` — `channel.Endpoint` (`gvisor.dev/gvisor/pkg/tcpip/
  link/channel`) вместо `tun.Tun`: пакеты ходят через `InjectInbound`/
  `ReadContext`, версия IP (4/6) определяется по старшему ниблу первого
  байта.
- `netunnel.go::StartTunnel` — вручную собирает `stack.Stack`
  (ipv4/ipv6 + tcp/udp протоколы), один NIC с `PromiscuousMode`/
  `Spoofing` (нужно для точки-точки "тоннель в одно лицо" — через
  единственный NIC идёt трафик к любым адресам назначения, не только к
  собственному IP), регистрирует `tcp.Forwarder`/`udp.Forwarder` вместо
  `tun.Handler`.
- `handler.go` — те же `tcp.ForwarderRequest`/`udp.ForwarderRequest` →
  `gonet.NewTCPConn`/`NewUDPConn` → `io.Copy`-relay в `HyClient.TCP`/
  `UDP()`, логика relay не изменилась, изменилась только точка входа
  (раньше sing-tun's `tun.Handler`, теперь gVisor forwarder).

**gvisor.dev/gvisor через `go get @latest` — ненадёжно, нужен
конкретный пин.** `@latest` зарезолвился в снэпшот с конфликтом
package-имён в `pkg/tcpip/stack` (`bridge_test.go` объявлен как
`package bridge_test`, а не `stack_test` — судя по всему, у них так
организовано во внутреннем Bazel-сборщике, который этого не замечает,
но `go build`/`go vet` ломает железно). Зафиксировано на коммите
`v0.0.0-20260224225140-573d5e7127a8` — это не угаданная версия, а
ровно то, что использует в проде `tailscale.com` (проверено через их
публичный `go.mod` на pkg.go.dev/proxy.golang.org). Также пришлось
поднять `go.work` апстрима hysteria2 с `go 1.24.0` до `1.26.4` —
gvisor этой версии требует более новый Go; `build.sh` теперь делает это
автоматически после клона (`go work edit -go=1.26.4`), сам файл не
наш и перегенерируется при каждой сборке заново.

Проверено: `go build`/`go vet ./netunnel/...` чисто, `gomobile bind
-target macos` собирает `.xcframework` без ошибок, `.appex` собирается
и подписывается в Xcode.

### Живое подключение — туннель реально работает; найден и исправлен последний баг (DNS)

Серия живых тестов нашла и закрыла ещё три проблемы подряд:

1. **Race condition в подтверждении статуса.** `wait_for_connect_result_blocking`
   читал `connection.status()` сразу после `startVPNTunnelAndReturnError()`
   и доверял этому значению так же, как статусам из уведомлений — система
   не успевала обновить статус с прошлого `.Disconnected` на
   `.Connecting`, и код мгновенно проваливал попытку на устаревшем
   статусе (~2.5с на каждой попытке, "Stop command received" сразу после
   "Calling startTunnelWithOptions" в логе расширения), сам же вызывая
   disconnect. Исправлено: начальный статус не считается провалом,
   только статус, пришедший ЧЕРЕЗ уведомление.
2. **`StartTunnel` иногда зависал на много дольше дефолтного
   `MaxIdleTimeout=30с`** без ошибки и без успеха (наблюдалось 90+с
   тишины). `client.NewClient` не принимает `context`, добавлен внешний
   таймаут 20с через горутину — best-effort страховка, не гарантия (сама
   горутина продолжает висеть в фоне, `context` туда не пробросить).
3. **DNS не резолвился через тоннель — главная находка.** Диагностика
   через `curl` (не `ping` — см. ниже) показала: `curl https://1.1.1.1`
   (по IP) через тоннель — успех, реальный HTTPS-ответ от Cloudflare
   через немецкий edge; `curl https://api.ipify.org` (по имени) —
   `Could not resolve host`. Причина: `NEPacketTunnelNetworkSettings`
   никогда не получал `dnsSettings` — системный резолвер продолжал
   слать запросы на оригинальный (обычно приватный) DNS, недостижимый
   через тоннель. **Исправлено**: `settings.dnsSettings = NEDNSSettings
   (servers: ["1.1.1.1", "8.8.8.8"])` в `PacketTunnelProvider.swift` —
   публичные резолверы достижимы через тот же `udpForwarderHandler`
   (для gVisor-стека порт 53 не отличается от любого другого UDP).

**Побочная находка при диагностике**: `ping` (ICMP) **всегда** будет
показывать "нет сети" через этот тоннель, даже когда всё реально
работает — наш gVisor-стек регистрирует только `tcp.NewProtocol`/
`udp.NewProtocol`, ICMP не поддержан и не обрабатывается вообще. Для
проверки реальной связности через тоннель использовать `curl`/`nc`, не
`ping`. Добавление ICMP в gVisor-стек (если понадобится для будущей
"проверки соединения" в UI) — отдельная, не критичная задача.

Проверено вживую: реальный hysteria2-сервер (`speed.vroxory.com:443`,
немецкий узел), реальный HTTPS-трафик через тоннель, подтверждённый
по `cf-ray` заголовку Cloudflare (Frankfurt edge — ожидаемо для
немецкого VPN-сервера).

### Код-ревью всего написанного за сессию по macOS NE — три реальные находки

После того как тоннель заработал, прогнан отдельный ревью-проход по
`netunnel/*.go`, `engine/macos.rs`, `PacketTunnelProvider.swift`. Найдено
и исправлено три проблемы стабильности (не блокеры — всё уже работало,
но накапливали бы утечки/мусор при длительном использовании):

1. **UDP-relay без idle-таймаута** (`netunnel/handler.go`). Каждый
   уникальный UDP-поток (DNS-запросы создают их пачками — на каждый
   запрос новый эфемерный порт) держал две горутины + gVisor-endpoint
   НАВСЕГДА, до полной остановки тоннеля — раньше эту логику закрывал
   `udpnat.New(udpTimeout, ...)` внутри sing-tun's System stack, при
   переходе на прямой gVisor (см. выше) она потерялась. Исправлено:
   `SetReadDeadline` на 60с, сбрасывается на каждый успешный пакет —
   тот же подход, что и был неявно через sing-tun.
2. **Наблюдатель за неожиданным разрывом тёрялся навсегда на каждый
   connect** (`engine/macos.rs::watch_for_unexpected_disconnect`). Был
   `mem::forget` и на блок, и на токен — за сессию с несколькими
   циклами connect/disconnect (как сегодня) накапливались наблюдатели,
   которые продолжали жить и реагировать на статусы уже совсем других
   соединений. Исправлено: наблюдатель сам себя снимает
   (`removeObserver_name_object`) при первом же реальном разрыве — токен
   хранится в `Arc<Mutex<>>`, не `Rc<RefCell<>>` (последний не годится:
   запись происходит на `spawn_blocking`-потоке, чтение/обнуление —
   внутри блока на главном потоке, а `RefCell`'овский флаг занятости не
   атомарный — настоящая гонка между двумя ОС-потоками, в отличие от
   самого objc2-объекта, который безопасен через ARC).
3. **`pumpInbound` без условия выхода** (`PacketTunnelProvider.swift`).
   В отличие от `pumpOutbound` (у которого есть `while let handle =
   tunnelHandle`), `pumpInbound` безусловно перевооружал себя
   (`self.pumpInbound()` в конце каждого вызова) даже после
   `stopTunnel()`/`tunnelHandle = nil`. Исправлено: проверка
   `tunnelHandle != nil` перед перевооружением.

Не исправлено осознанно (не блокер, отдельная задача при необходимости):
`channel.Endpoint`'s исходящая очередь на 256 пакетов молча роняет
новые пакеты при переполнении (`channel.go::queue.Write`, `default:
p.DecRef()`) — нет ни backpressure, ни логирования дропов. Под обычной
нагрузкой не проявлялось, но при устойчиво высоком throughput могло бы
давать необъяснимые потери пакетов без единого следа в логах.
