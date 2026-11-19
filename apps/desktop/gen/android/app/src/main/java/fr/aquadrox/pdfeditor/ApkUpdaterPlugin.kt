package fr.aquadrox.pdfeditor

import android.Manifest
import android.app.Activity
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.provider.Settings
import android.webkit.WebView
import androidx.activity.result.ActivityResult
import androidx.appcompat.app.AppCompatActivity
import app.tauri.PermissionState
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.Permission
import app.tauri.annotation.PermissionCallback
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Channel
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File

@InvokeArg
class InstallArgs {
  lateinit var path: String
  lateinit var version: String
  var results: Channel? = null
}

/**
 * Mise à jour Android (A-04), appelée par Rust (`apps/desktop/src/apk_installer.rs`) :
 * `prepare` obtient les autorisations et donne l'emplacement de l'APK,
 * `install` le confie à Android ([ApkInstaller]).
 */
@TauriPlugin(
  permissions = [Permission(strings = [Manifest.permission.POST_NOTIFICATIONS], alias = "notifications")],
)
class ApkUpdaterPlugin(private val activity: Activity) : Plugin(activity) {
  /**
   * Avant le téléchargement : autorisation « installer des applications »
   * (écran des réglages d'Android, la mise à jour reprend au retour), puis,
   * une seule fois, celle des notifications (pour rouvrir PdfEditor après
   * la mise à jour). Renvoie `{ apk }`, où Rust doit ranger le paquet.
   */
  @Command
  fun prepare(invoke: Invoke) {
    if (!ApkInstaller.supported) {
      invoke.reject("version de test : installez l'APK d'une release pour recevoir les mises à jour")
      return
    }
    if (!canInstall()) {
      val intent = Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:${activity.packageName}"))
      startActivityForResult(invoke, intent, "installPermissionResult")
      return
    }
    askNotifications(invoke)
  }

  @ActivityCallback
  private fun installPermissionResult(invoke: Invoke, @Suppress("UNUSED_PARAMETER") result: ActivityResult) {
    if (!canInstall()) {
      invoke.reject("PdfEditor n'est pas autorisé à installer des applications : autorisez-le, puis touchez à nouveau « Mettre à jour »")
      return
    }
    askNotifications(invoke)
  }

  private fun canInstall(): Boolean =
    Build.VERSION.SDK_INT < Build.VERSION_CODES.O || activity.packageManager.canRequestPackageInstalls()

  private fun askNotifications(invoke: Invoke) {
    val prefs = activity.getSharedPreferences("updates", Context.MODE_PRIVATE)
    val asked = prefs.getBoolean("notifications_asked", false)
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU && !asked &&
      getPermissionState("notifications") != PermissionState.GRANTED
    ) {
      prefs.edit().putBoolean("notifications_asked", true).apply()
      requestPermissionForAlias("notifications", invoke, "notificationsResult")
      return
    }
    resolvePrepared(invoke)
  }

  /** Notifications refusées : la mise à jour se fait quand même. */
  @PermissionCallback
  private fun notificationsResult(invoke: Invoke) {
    resolvePrepared(invoke)
  }

  private fun resolvePrepared(invoke: Invoke) {
    val dir = ApkInstaller.updateDir(activity)
    invoke.resolve(JSObject().put("apk", File(dir, "PdfEditor.apk").path))
  }

  /** APK téléchargé et vérifié par Rust : relu puis confié à Android. */
  @Command
  fun install(invoke: Invoke) {
    val args = invoke.parseArgs(InstallArgs::class.java)
    ApkInstaller.results = args.results
    // Appelé juste après un appui sur « Mettre à jour » : écran affiché.
    ApkInstaller.visible = true
    val context = activity.applicationContext
    Thread {
      try {
        val apk = ApkInstaller.checkedApk(context, args.path)
        ApkInstaller.inspect(context, apk, args.version)
        ApkInstaller.install(context, apk, silent = true)
        invoke.resolve()
      } catch (error: Exception) {
        ApkInstaller.discard(context)
        invoke.reject(error.message ?: error.toString())
      }
    }.start()
  }

  /** Chargé avec l'écran : PdfEditor est affiché. */
  override fun load(webView: WebView) {
    ApkInstaller.visible = true
  }

  override fun onResume(activity: AppCompatActivity) {
    ApkInstaller.visible = true
    // Confirmation d'Android arrivée pendant que PdfEditor était caché.
    ApkInstaller.pendingConfirm?.let { confirm ->
      ApkInstaller.pendingConfirm = null
      runCatching { activity.startActivity(confirm) }
    }
  }

  override fun onPause(activity: AppCompatActivity) {
    ApkInstaller.visible = false
  }
}
