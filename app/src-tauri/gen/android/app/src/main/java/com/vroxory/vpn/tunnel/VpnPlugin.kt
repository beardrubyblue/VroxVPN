package com.vroxory.vpn.tunnel

import android.Manifest
import android.app.Activity
import android.content.pm.PackageManager
import android.net.VpnService
import android.os.Build
import androidx.activity.result.ActivityResult
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import kotlin.concurrent.thread

// Мост Rust → VpnService (engine/android/plugin.rs): start / stop / stats.
// Отдельной библиотекой-плагином не оформлен — Tauri находит класс через
// загрузчик классов activity, регистрация в Rust (register_android_plugin).
@TauriPlugin
class VpnPlugin(private val activity: Activity) : Plugin(activity) {

  @Command
  fun start(invoke: Invoke) {
    requestNotificationPermission()
    // первый connect: системный диалог «разрешить VPN-подключение»
    val consent = VpnService.prepare(activity)
    if (consent != null) startActivityForResult(invoke, consent, "onVpnConsent")
    else launch(invoke)
  }

  @ActivityCallback
  fun onVpnConsent(invoke: Invoke, result: ActivityResult) {
    if (result.resultCode == Activity.RESULT_OK) launch(invoke)
    else invoke.reject("VPN permission denied")
  }

  private fun launch(invoke: Invoke) {
    val args = invoke.parseArgs(StartArgs::class.java)
    VroxVpnService.start(activity, args) { error ->
      if (error == null) invoke.resolve(JSObject()) else invoke.reject(error)
    }
  }

  @Command
  fun stop(invoke: Invoke) {
    // Go Stop закрывает QUIC-соединение — не на главном потоке
    thread(name = "vrox-disconnect") {
      VroxVpnService.stop(activity)
      invoke.resolve(JSObject())
    }
  }

  @Command
  fun stats(invoke: Invoke) {
    // нет ключа stats — тоннель не запущен (Rust: Option::None)
    invoke.resolve(JSObject().put("stats", VroxVpnService.stats()))
  }

  // Android 13+: без разрешения постоянное уведомление VPN скрыто.
  // Не ждём ответа — VPN работает и без него.
  private fun requestNotificationPermission() {
    if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return
    val permission = Manifest.permission.POST_NOTIFICATIONS
    if (activity.checkSelfPermission(permission) != PackageManager.PERMISSION_GRANTED) {
      activity.requestPermissions(arrayOf(permission), 0)
    }
  }
}
