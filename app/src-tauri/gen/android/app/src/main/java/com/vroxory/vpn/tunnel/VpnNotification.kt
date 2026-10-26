package com.vroxory.vpn.tunnel

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import com.vroxory.vpn.MainActivity
import com.vroxory.vpn.R

// Постоянное уведомление foreground-сервиса: пока VPN включён, Android
// требует его показывать. Тап открывает приложение, «Disconnect» — стоп.
object VpnNotification {
  const val ID = 1
  private const val CHANNEL_ID = "vpn"

  fun build(context: Context, sessionName: String): Notification {
    val manager = context.getSystemService(NotificationManager::class.java)
    // IMPORTANCE_LOW — без звука и всплывания, просто строка в шторке
    manager.createNotificationChannel(
      NotificationChannel(CHANNEL_ID, "VPN connection", NotificationManager.IMPORTANCE_LOW)
    )
    val open = PendingIntent.getActivity(
      context, 0,
      Intent(context, MainActivity::class.java),
      PendingIntent.FLAG_IMMUTABLE
    )
    val disconnect = PendingIntent.getService(
      context, 0,
      Intent(context, VroxVpnService::class.java).setAction(VroxVpnService.ACTION_STOP),
      PendingIntent.FLAG_IMMUTABLE
    )
    return Notification.Builder(context, CHANNEL_ID)
      .setSmallIcon(R.mipmap.ic_launcher)
      .setContentTitle("vrox.vpn")
      .setContentText("Connected · $sessionName")
      .setContentIntent(open)
      .addAction(Notification.Action.Builder(null, "Disconnect", disconnect).build())
      .setOngoing(true)
      .build()
  }
}
