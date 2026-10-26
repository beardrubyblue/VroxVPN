package com.vroxory.vpn.tunnel

import app.tauri.annotation.InvokeArg

// Аргументы команды start — поля 1:1 с engine/android/plugin.rs::StartArgs.
@InvokeArg
class StartArgs {
  // JSON для netunnel.StartFD (тот же формат, что у iOS-расширения)
  lateinit var configJson: String
  // CIDR интерфейса
  lateinit var inet4Addr: String
  // только hysteria2: с ним весь IPv6 тоже в тоннеле (иначе утечка мимо VPN)
  var inet6Addr: String? = null
  var mtu: Int = 1500
  // маршруты мимо тоннеля: сервер, локальные сети, RU-обход
  var ipv4Exclude: List<String> = emptyList()
  var ipv6Exclude: List<String> = emptyList()
  // имя узла — в постоянное уведомление
  var sessionName: String = ""
}
