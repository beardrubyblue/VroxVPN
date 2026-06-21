import Cocoa

// Явный bootstrap вместо @NSApplicationMain/@main — синтез делегата через
// эти атрибуты не сработал на этой связке Xcode/Swift (applicationDidFinish
// Launching не вызывался ни разу, подтверждено файловым лог-выводом),
// похоже не присваивает AppDelegate как NSApp.delegate автоматически. Этот
// явный вариант — стандартный, документированный способ без сюрпризов.
let delegate = AppDelegate()
NSApplication.shared.delegate = delegate
_ = NSApplicationMain(CommandLine.argc, CommandLine.unsafeArgv)
