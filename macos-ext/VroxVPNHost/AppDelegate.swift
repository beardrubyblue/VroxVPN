import Cocoa
import NetworkExtension

/// Голый тест-харнесс для Фазы 2 плана (см. docs/ARCHITECTURE.md,
/// раздел macOS/NetworkExtension): только подтвердить, что
/// VroxTunnelExtension реально поднимает тоннель и пропускает трафик
/// через netunnel. Никакого реального UI/Tauri-интеграции здесь нет —
/// это отдельная задача (control-bridge в engine/macos.rs, Фаза 6).
class AppDelegate: NSObject, NSApplicationDelegate {
    var window: NSWindow!
    var manager: NETunnelProviderManager?

    func applicationDidFinishLaunching(_ notification: Notification) {
        window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 420, height: 160),
            styleMask: [.titled, .closable],
            backing: .buffered,
            defer: false
        )
        window.title = "VroxVPN NE test harness"
        window.center()

        let connectButton = NSButton(title: "Connect (test config)", target: self, action: #selector(connect))
        connectButton.frame = NSRect(x: 20, y: 90, width: 380, height: 30)
        let disconnectButton = NSButton(title: "Disconnect", target: self, action: #selector(disconnect))
        disconnectButton.frame = NSRect(x: 20, y: 50, width: 380, height: 30)

        let contentView = NSView(frame: window.contentRect(forFrameRect: window.frame))
        contentView.addSubview(connectButton)
        contentView.addSubview(disconnectButton)
        window.contentView = contentView
        NSApp.setActivationPolicy(.regular)
        NSApp.activate(ignoringOtherApps: true)
        window.makeKeyAndOrderFront(nil)
    }

    /// configJSON ожидается в формате netunnel.Config (см.
    /// packaging/hysteria2-patch/netunnel/netunnel.go) — для реального
    /// теста подставить настоящий server/auth тестового hysteria2-сервера.
    /// При интеграции с control-bridge (engine/macos.rs) это поле будет
    /// приходить из config_gen::generate_provider_config_json, а не быть
    /// захардкоженным здесь.
    @objc func connect() {
        let testConfigJSON = """
        {"server":"127.0.0.1:1","auth":"test","sni":"example.com","insecure":true,
         "obfs":{"type":"","salamander":{"password":""}},
         "bandwidth":{"up":"","down":""},"congestion":{"type":"","bbrProfile":""},
         "inet4Addr":"100.100.100.101/30","mtu":1500}
        """

        let proto = NETunnelProviderProtocol()
        proto.providerBundleIdentifier = "com.vroxory.vpn.tunnel"
        proto.serverAddress = "vrox.vpn"
        // killswitch под NE — см. PacketTunnelProvider.swift и
        // docs/ARCHITECTURE.md, Фаза 4: includeAllNetworks свойство
        // именно протокола конфигурации (хост), а не
        // NEPacketTunnelNetworkSettings (расширение).
        proto.includeAllNetworks = true
        proto.providerConfiguration = [
            "configJSON": testConfigJSON,
            "inet4Addr": "100.100.100.101",
            "mtu": 1500,
        ]

        NETunnelProviderManager.loadAllFromPreferences { [weak self] managers, error in
            guard let self else { return }
            let manager = managers?.first ?? NETunnelProviderManager()
            manager.protocolConfiguration = proto
            manager.localizedDescription = "vrox.vpn (NE test)"
            manager.isEnabled = true

            manager.saveToPreferences { saveError in
                if let saveError {
                    NSLog("saveToPreferences ошибка: %@", saveError.localizedDescription)
                    return
                }
                manager.loadFromPreferences { loadError in
                    if let loadError {
                        NSLog("loadFromPreferences ошибка: %@", loadError.localizedDescription)
                        return
                    }
                    self.manager = manager
                    do {
                        try manager.connection.startVPNTunnel()
                    } catch {
                        NSLog("startVPNTunnel ошибка: %@", error.localizedDescription)
                    }
                }
            }
        }
    }

    @objc func disconnect() {
        manager?.connection.stopVPNTunnel()
    }
}
