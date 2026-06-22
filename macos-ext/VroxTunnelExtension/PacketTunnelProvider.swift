import GoNetunnel
import NetworkExtension
import os.log

/// Хост для netunnel (см. packaging/hysteria2-patch/netunnel/) —
/// gVisor-стек + hysteria2-клиент без настоящего TUN-устройства, собран
/// через `gomobile bind` в GoNetunnel.xcframework (см. ../build-go-
/// framework.sh). Здесь — только relay: пакеты из packetFlow в Go,
/// пакеты из Go в packetFlow. Сам протокол/TLS/QUIC полностью внутри Go.
///
/// Вызовы в Go-биндинг используют `try`, а не явный `NSErrorPointer` —
/// Swift-импортер Objective-C автоматически мостит методы вида
/// `- (T)foo:(NSError**)error` (NSError** последним параметром,
/// nullable-результат как индикатор неудачи) в `func foo() throws -> T`
/// (см. сгенерированный Netunnel.objc.h).
class PacketTunnelProvider: NEPacketTunnelProvider {
    private var tunnelHandle: NetunnelTunnelHandle?
    private let log = OSLog(subsystem: "com.vroxory.vpn.tunnel", category: "PacketTunnelProvider")

    override func startTunnel(
        options: [String: NSObject]?,
        completionHandler: @escaping (Error?) -> Void
    ) {
        guard let protocolConfig = self.protocolConfiguration as? NETunnelProviderProtocol,
              let providerConfig = protocolConfig.providerConfiguration,
              let configJSON = providerConfig["configJSON"] as? String
        else {
            completionHandler(NSError(
                domain: "com.vroxory.vpn.tunnel",
                code: 1,
                userInfo: [NSLocalizedDescriptionKey: "providerConfiguration.configJSON отсутствует"]
            ))
            return
        }

        // NetunnelStartTunnel — свободная C-функция (FOUNDATION_EXPORT), а
        // не Objective-C метод: автомост NSError** → throws у Swift-
        // импортёра действует только для ObjC-методов класса (writePacket/
        // readPacket/stop ниже — методы, поэтому там `try` работает),
        // для свободных функций нужен явный NSErrorPointer.
        var startErr: NSError?
        guard let handle = NetunnelStartTunnel(configJSON, &startErr) else {
            let error = startErr ?? NSError(
                domain: "com.vroxory.vpn.tunnel",
                code: 2,
                userInfo: [NSLocalizedDescriptionKey: "StartTunnel вернул nil без ошибки"]
            )
            os_log("netunnel.StartTunnel завершился ошибкой: %{public}@", log: log, type: .error, error.localizedDescription)
            completionHandler(error)
            return
        }
        self.tunnelHandle = handle

        // Сетевые настройки тоннеля. killswitch под NE (см.
        // docs/ARCHITECTURE.md, Фаза 4) — это includedRoutes=[default]
        // ниже, не includeAllNetworks (тот специально убран — подтверждено
        // вживую, что он блокирует и собственный трафик тоннеля до того,
        // как тоннель поднялся, см. ARCHITECTURE.md). Весь трафик идёт
        // через тоннель по умолчанию, без отдельного pf-ruleset, который
        // был нужен в удалённом sidecar-пути.
        // excludedRoutes (сервер + приватные диапазоны + RU-geoip) придут
        // в providerConfig отдельным полем — TODO при первой интеграции
        // с config_gen::generate_excluded_routes (см. control-bridge).
        // inet4Addr приходит как CIDR ("100.100.100.101/30", формат
        // config_gen.rs::generate_provider_config_json) — NEIPv4Settings/
        // tunnelRemoteAddress ждут чистый адрес без префикса, без этого
        // падает с "Invalid IPv4 address"/"Invalid NETunnelNetworkSettings
        // tunnelRemoteAddress" (проверено вживую).
        let inet4CIDR = (providerConfig["inet4Addr"] as? String) ?? "100.100.100.101/30"
        let inet4 = inet4CIDR.split(separator: "/").first.map(String.init) ?? inet4CIDR
        // tunnelRemoteAddress — адрес самого VPN-сервера (для идентификации
        // тоннеля системой), не наш виртуальный локальный IP; берём host
        // из providerConfig["server"] ("host:port"), с фоллбеком на inet4,
        // если поля нет.
        let remoteAddress = (providerConfig["server"] as? String)
            .flatMap { $0.split(separator: ":").first.map(String.init) } ?? inet4
        let settings = NEPacketTunnelNetworkSettings(tunnelRemoteAddress: remoteAddress)
        settings.ipv4Settings = NEIPv4Settings(addresses: [inet4], subnetMasks: ["255.255.255.252"])
        settings.ipv4Settings?.includedRoutes = [NEIPv4Route.default()]
        settings.mtu = (providerConfig["mtu"] as? NSNumber) ?? 1500
        // Без dnsSettings DNS-запросы продолжают идти на оригинальный
        // (обычно приватный, типа 192.168.x.x) резолвер системы — он
        // недостижим через тоннель, и резолвинг по имени просто не
        // работает (проверено вживую: curl по IP — успех, curl по имени
        // — "Could not resolve host", при реально поднятом и рабочем
        // тоннеле). Публичные DNS реально резолвятся через тот же
        // udpForwarderHandler (netunnel/handler.go) — для стека порт 53
        // ничем не отличается от любого другого UDP-трафика.
        settings.dnsSettings = NEDNSSettings(servers: ["1.1.1.1", "8.8.8.8"])

        self.setTunnelNetworkSettings(settings) { [weak self] error in
            guard let self else { return }
            if let error {
                os_log("setTunnelNetworkSettings ошибка: %{public}@", log: self.log, type: .error, error.localizedDescription)
                completionHandler(error)
                return
            }
            self.startPacketPump()
            completionHandler(nil)
        }
    }

    /// Два независимых цикла: пакеты ОТ ОС (через packetFlow) — в Go, и
    /// пакеты ОТ Go (gVisor-стек) — обратно в ОС. packetFlow.readPackets
    /// устроен как self-re-armed callback (не цикл while), поэтому для
    /// входящих пакетов используем именно этот API.
    private func startPacketPump() {
        pumpInbound()
        pumpOutbound()
    }

    private func pumpInbound() {
        // У pumpOutbound есть естественное условие выхода (`while let
        // handle = tunnelHandle`, плюс readPacket() сам бросает после
        // Stop()) — у этого цикла такого не было: он перевооружал себя
        // безусловно, даже после stopTunnel()/tunnelHandle=nil. Если
        // packetFlow продолжит вызывать callback (расширение ещё не
        // деаллоцировано) — цикл крутился бы вечно вхолостую.
        guard tunnelHandle != nil else { return }
        packetFlow.readPackets { [weak self] packets, _ in
            guard let self, self.tunnelHandle != nil else { return }
            for packet in packets {
                do {
                    try self.tunnelHandle?.writePacket(packet)
                } catch {
                    os_log("WritePacket ошибка: %{public}@", log: self.log, type: .error, error.localizedDescription)
                }
            }
            self.pumpInbound()
        }
    }

    /// ReadPacket — блокирующий Go-вызов, поэтому крутим его в отдельном
    /// фоновом потоке (а не на потоке packetFlow.readPackets'а выше),
    /// иначе один цикл заблокировал бы другой.
    private func pumpOutbound() {
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self else { return }
            while let handle = self.tunnelHandle {
                do {
                    let pkt = try handle.readPacket()
                    // Версия IP — верхний нибл первого байта (4 или 6),
                    // стандартный способ отличить v4/v6 без парсинга
                    // всего заголовка. packetFlow.writePackets требует
                    // явный протокол на КАЖДЫЙ пакет — нельзя просто
                    // считать, что весь трафик IPv4 (gVisor-стек у нас
                    // настроен на оба address family, см. netunnel.go
                    // StartTunnel: inet6Prefixes).
                    let proto: Int32 = (pkt.first.map { $0 >> 4 } == 6) ? AF_INET6 : AF_INET
                    self.packetFlow.writePackets([pkt], withProtocols: [proto as NSNumber])
                } catch {
                    os_log("ReadPacket завершился: %{public}@", log: self.log, type: .info, error.localizedDescription)
                    return // Stop() закрыл vtun (EOF) — выходим из цикла
                }
            }
        }
    }

    override func stopTunnel(with reason: NEProviderStopReason, completionHandler: @escaping () -> Void) {
        try? tunnelHandle?.stop()
        tunnelHandle = nil
        completionHandler()
    }

    override func handleAppMessage(_ messageData: Data, completionHandler: ((Data?) -> Void)?) {
        completionHandler?(nil)
    }
}
