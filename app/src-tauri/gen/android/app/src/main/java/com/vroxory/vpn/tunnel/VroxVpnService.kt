package com.vroxory.vpn.tunnel

import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.net.IpPrefix
import android.net.VpnService
import android.os.Build
import com.vroxory.netunnel.Netunnel
import com.vroxory.netunnel.Tunnel
import java.net.InetAddress
import kotlin.concurrent.thread

// VPN-сервис: поднимает TUN через VpnService.Builder и отдаёт fd в Go
// (netunnel.StartFD — пакеты качает Go). Живёт в процессе приложения,
// поэтому VpnPlugin достаёт тоннель напрямую через companion.
class VroxVpnService : VpnService() {
  companion object {
    const val ACTION_STOP = "com.vroxory.vpn.STOP"
    private val DNS_SERVERS = listOf("1.1.1.1", "1.0.0.1")

    @Volatile private var tunnel: Tunnel? = null
    // запрос на старт и его колбэк: VpnService.Builder есть только у
    // запущенного экземпляра сервиса, поэтому плагин кладёт их сюда и
    // стартует сервис, а onStartCommand забирает
    private var pending: Pair<StartArgs, (String?) -> Unit>? = null

    fun start(context: Context, args: StartArgs, onResult: (String?) -> Unit) {
      synchronized(this) { pending = args to onResult }
      context.startForegroundService(Intent(context, VroxVpnService::class.java))
    }

    fun stop(context: Context) {
      stopTunnel()
      context.stopService(Intent(context, VroxVpnService::class.java))
    }

    fun stats(): String? = tunnel?.stats

    @Synchronized private fun stopTunnel() {
      val current = tunnel ?: return
      tunnel = null
      // Stop дописывает хвост трафика в историю и закрывает fd
      runCatching { current.stop() }
    }
  }

  override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
    if (intent?.action == ACTION_STOP) {
      stop(this)
      return START_NOT_STICKY
    }
    val (args, onResult) = synchronized(Companion) { pending.also { pending = null } }
      ?: return START_NOT_STICKY
    // startForeground обязан прозвучать в течение нескольких секунд после
    // startForegroundService — до долгого подключения к серверу
    val notification = VpnNotification.build(this, args.sessionName)
    // тип systemExempted (VPN) обязателен и существует только с Android 14
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
      startForeground(VpnNotification.ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_SYSTEM_EXEMPTED)
    } else {
      startForeground(VpnNotification.ID, notification)
    }
    // подключение к серверу (QUIC / рукопожатие WireGuard) — не на главном потоке
    thread(name = "vrox-connect") {
      val error = runCatching { connect(args) }.exceptionOrNull()
      if (error != null) stop(this)
      onResult(error?.let { it.message ?: it.toString() })
    }
    return START_NOT_STICKY
  }

  private fun connect(args: StartArgs) {
    stopTunnel()
    val builder = Builder()
      .setSession(args.sessionName)
      .setMtu(args.mtu)
      // собственный трафик приложения (Go-сокет к серверу, подписки, пинг)
      // идёт мимо тоннеля — иначе тоннель завернул бы сам себя
      .addDisallowedApplication(packageName)
    addAddress(builder, args.inet4Addr)
    builder.addRoute("0.0.0.0", 0)
    args.inet6Addr?.let {
      addAddress(builder, it)
      builder.addRoute("::", 0)
    }
    // excludeRoute — только Android 13+. На 12 обход RU и локальной сети
    // не работает (всё идёт в тоннель); сервер и так мимо — через
    // addDisallowedApplication выше
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
      args.ipv4Exclude.forEach { builder.excludeRoute(parsePrefix(it)) }
      if (args.inet6Addr != null) args.ipv6Exclude.forEach { builder.excludeRoute(parsePrefix(it)) }
    }
    DNS_SERVERS.forEach { builder.addDnsServer(it) }

    val tun = builder.establish() ?: error("VPN permission revoked")
    // detachFd — fd переходит во владение Go, закрывается в Tunnel.stop()
    tunnel = Netunnel.startFD(args.configJson, tun.detachFd().toLong())
  }

  private fun addAddress(builder: Builder, cidr: String) {
    val (address, prefix) = cidr.split("/").let { it[0] to (it.getOrNull(1)?.toInt() ?: 32) }
    builder.addAddress(address, prefix)
  }

  private fun parsePrefix(cidr: String): IpPrefix {
    val (address, prefix) = cidr.split("/").let { it[0] to it[1].toInt() }
    return IpPrefix(InetAddress.getByName(address), prefix)
  }

  // ОС отозвала VPN (включили другое VPN-приложение): Rust заметит по stats
  override fun onRevoke() {
    stop(this)
  }

  override fun onDestroy() {
    stopTunnel()
    super.onDestroy()
  }
}
