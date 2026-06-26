import NetworkExtension
import SwiftUI

/// Та же логика, что в VroxVPNHost/AppDelegate.swift (см. doc-комментарий
/// там про includeAllNetworks) — NETunnelProviderManager/NEVPNConnection
/// API идентичен на iOS и macOS, переносится без изменений, меняется
/// только обёртка вокруг него (SwiftUI вместо NSWindow/NSButton).
final class TunnelController: ObservableObject {
    @Published var statusText = "не подключено"
    private var manager: NETunnelProviderManager?

    /// configJSON ожидается в формате netunnel.Config (см.
    /// packaging/hysteria2-patch/netunnel/netunnel.go) — для реального
    /// теста подставить настоящий server/auth тестового hysteria2-сервера.
    func connect() {
        let testConfigJSON = """
        {"server":"127.0.0.1:1","auth":"test","sni":"example.com","insecure":true,
         "obfs":{"type":"","salamander":{"password":""}},
         "bandwidth":{"up":"","down":""},"congestion":{"type":"","bbrProfile":""},
         "inet4Addr":"100.100.100.101/30","mtu":1500}
        """

        let proto = NETunnelProviderProtocol()
        proto.providerBundleIdentifier = "com.vroxory.vpn.tunnel"
        proto.serverAddress = "vrox.vpn"
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
                    self.statusText = "saveToPreferences: \(saveError.localizedDescription)"
                    return
                }
                manager.loadFromPreferences { loadError in
                    if let loadError {
                        self.statusText = "loadFromPreferences: \(loadError.localizedDescription)"
                        return
                    }
                    self.manager = manager
                    do {
                        try manager.connection.startVPNTunnel()
                        self.statusText = "подключение..."
                    } catch {
                        self.statusText = "startVPNTunnel: \(error.localizedDescription)"
                    }
                }
            }
        }
    }

    func disconnect() {
        manager?.connection.stopVPNTunnel()
        statusText = "не подключено"
    }
}

struct ContentView: View {
    @StateObject private var controller = TunnelController()

    var body: some View {
        VStack(spacing: 16) {
            Text("VroxVPN NE test harness")
                .font(.headline)
            Text(controller.statusText)
                .foregroundStyle(.secondary)
            Button("Connect (test config)") { controller.connect() }
                .buttonStyle(.borderedProminent)
            Button("Disconnect") { controller.disconnect() }
                .buttonStyle(.bordered)
        }
        .padding()
    }
}
