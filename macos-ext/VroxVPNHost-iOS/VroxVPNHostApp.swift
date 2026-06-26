import SwiftUI

/// iOS/iPadOS-вариант тест-харнесса (см. VroxVPNHost/AppDelegate.swift —
/// тот же смысл, но на AppKit, который на iOS не собирается вообще).
/// SwiftUI App lifecycle выбран не из вкуса — на нём не воспроизводится
/// проблема "@main/@NSApplicationMain не вызывает applicationDidFinish
/// Launching", задокументированная в VroxVPNHost/main.swift: это другой
/// механизм запуска (SwiftUI сам управляет жизненным циклом, не зависит
/// от ENABLE_DEBUG_DYLIB), поэтому здесь @main работает напрямую.
@main
struct VroxVPNHostApp: App {
    var body: some Scene {
        WindowGroup {
            ContentView()
        }
    }
}
