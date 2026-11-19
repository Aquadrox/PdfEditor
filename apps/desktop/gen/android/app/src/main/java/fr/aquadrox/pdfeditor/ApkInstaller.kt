package fr.aquadrox.pdfeditor

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.pm.PackageInfo
import android.content.pm.PackageInstaller
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.content.pm.PackageInfoCompat
import app.tauri.plugin.Channel
import app.tauri.plugin.JSObject
import java.io.File

/**
 * Mise à jour de l'application (A-04), reprise de VoiceToText (`Updater.kt`).
 * Rust a déjà téléchargé l'APK et vérifié sa taille et son SHA-256 publiés ;
 * ici, le paquet est relu (identifiant, version, signature) puis confié à
 * l'installateur d'Android.
 *
 * Installation sans écran de confirmation quand le téléphone le permet
 * (Android 12 et plus : le clic « Mettre à jour » a déjà eu lieu). Xiaomi
 * (HyperOS) refuse d'office ce cas (« INSTALL_FAILED_ABORTED: Permission
 * denied ») : l'APK est gardé jusqu'au résultat et, sur ce refus, renvoyé
 * aussitôt avec la confirmation d'Android. Sans cela, il fallait toucher
 * « Mettre à jour » deux fois (le second essai, moins de 30 s après, passait
 * parce qu'Android demande alors la confirmation de lui-même).
 */
object ApkInstaller {
  private const val CHANNEL = "updates"
  private const val NOTIFY_UPDATED = 7001
  const val EXTRA_SILENT = "fr.aquadrox.pdfeditor.SILENT_UPDATE"
  const val EXTRA_APK = "fr.aquadrox.pdfeditor.UPDATE_APK"

  /** Résultats envoyés à Rust (fil donné à chaque installation). */
  @Volatile
  var results: Channel? = null

  /** Écran de PdfEditor affiché : la confirmation d'Android peut s'ouvrir. */
  @Volatile
  var visible = false

  /** Confirmation arrivée application cachée : ouverte au retour à l'écran. */
  @Volatile
  var pendingConfirm: Intent? = null

  /** Version de test (PR) : autre identifiant et autre clé, pas de mise à jour. */
  val supported: Boolean
    get() = !BuildConfig.DEBUG

  /** Dossier où Rust range l'APK : le cache de l'application. */
  fun updateDir(context: Context) = File(context.cacheDir, "update")

  /** APK donné par Rust, accepté seulement dans le dossier des mises à jour. */
  fun checkedApk(context: Context, path: String): File {
    val file = File(path).canonicalFile
    val dir = updateDir(context).canonicalFile
    check(file.parentFile == dir && file.isFile) { "Paquet introuvable : téléchargez à nouveau." }
    return file
  }

  /** Relit l'APK : même application, version annoncée et plus récente, même signature. */
  fun inspect(context: Context, file: File, version: String) {
    val pm = context.packageManager
    val archive = archiveInfo(pm, file) ?: error("Paquet illisible : fichier refusé.")
    check(archive.packageName == context.packageName) { "Ce paquet n'est pas PdfEditor : refusé." }
    check(archive.versionName == version) { "Version du paquet différente de celle annoncée : refusé." }
    val installed = installedInfo(pm, context.packageName)
    check(PackageInfoCompat.getLongVersionCode(archive) > PackageInfoCompat.getLongVersionCode(installed)) {
      "Ce paquet n'est pas plus récent que l'application installée : refusé."
    }
    // Android revérifie la signature à l'installation ; ce contrôle donne
    // seulement un message clair. Signature non lue : Android jugera.
    if (Build.VERSION.SDK_INT < Build.VERSION_CODES.P) return
    val incoming = archive.signingInfo ?: return
    val mine = installed.signingInfo?.apkContentsSigners?.toSet() ?: return
    val theirs = incoming.apkContentsSigners.toSet()
    val rotated = incoming.signingCertificateHistory.orEmpty().toSet().containsAll(mine)
    check(theirs == mine || rotated) { "Paquet signé avec une autre clé que l'application installée : refusé." }
  }

  private fun signingFlag(): Int =
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) PackageManager.GET_SIGNING_CERTIFICATES else 0

  private fun archiveInfo(pm: PackageManager, file: File): PackageInfo? =
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
      pm.getPackageArchiveInfo(file.path, PackageManager.PackageInfoFlags.of(signingFlag().toLong()))
    } else {
      @Suppress("DEPRECATION")
      pm.getPackageArchiveInfo(file.path, signingFlag())
    }

  private fun installedInfo(pm: PackageManager, name: String): PackageInfo =
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
      pm.getPackageInfo(name, PackageManager.PackageInfoFlags.of(signingFlag().toLong()))
    } else {
      @Suppress("DEPRECATION")
      pm.getPackageInfo(name, signingFlag())
    }

  /**
   * Confie l'APK vérifié à l'installateur d'Android ; le résultat arrive à
   * [UpdateResultReceiver]. `silent` : sans confirmation (Android 12 et
   * plus) ; l'APK est alors gardé pour réessayer avec confirmation si le
   * téléphone la refuse. Hors du fil principal (copie de l'APK).
   */
  fun install(context: Context, file: File, silent: Boolean) {
    val installer = context.packageManager.packageInstaller
    val quiet = silent && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S
    val params = PackageInstaller.SessionParams(PackageInstaller.SessionParams.MODE_FULL_INSTALL).apply {
      setAppPackageName(context.packageName)
      setSize(file.length())
      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
        setRequireUserAction(
          if (quiet) PackageInstaller.SessionParams.USER_ACTION_NOT_REQUIRED
          else PackageInstaller.SessionParams.USER_ACTION_REQUIRED,
        )
      }
    }
    var keep = false
    val id = installer.createSession(params)
    try {
      installer.openSession(id).use { session ->
        session.openWrite("PdfEditor.apk", 0, file.length()).use { output ->
          file.inputStream().use { it.copyTo(output) }
          session.fsync(output)
        }
        val intent = Intent(context, UpdateResultReceiver::class.java)
          .putExtra(EXTRA_SILENT, quiet)
          .putExtra(EXTRA_APK, file.path)
        // Mutable : Android y ajoute le statut de l'installation.
        val mutable = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) PendingIntent.FLAG_MUTABLE else 0
        val pending = PendingIntent.getBroadcast(context, id, intent, PendingIntent.FLAG_UPDATE_CURRENT or mutable)
        session.commit(pending.intentSender)
        keep = quiet
      }
    } catch (error: Exception) {
      runCatching { installer.abandonSession(id) }
      throw error
    } finally {
      // Android a copié le paquet dans la session ; gardé seulement pour un
      // second essai avec confirmation.
      if (!keep) discard(context)
    }
  }

  /** Efface l'APK téléchargé (plus utile). */
  fun discard(context: Context) {
    updateDir(context).deleteRecursively()
  }

  /** Prévient Rust : `state` = confirm (fenêtre d'Android), installed, failed. */
  fun report(state: String, message: String? = null) {
    val data = JSObject().put("state", state)
    if (message != null) data.put("message", message)
    runCatching { results?.send(data) }
  }

  /** Message pour un statut d'échec de l'installateur d'Android. */
  fun failure(status: Int, detail: String?): String {
    val reason = when (status) {
      PackageInstaller.STATUS_FAILURE_ABORTED -> "installation annulée"
      PackageInstaller.STATUS_FAILURE_BLOCKED -> "installation bloquée par le téléphone"
      PackageInstaller.STATUS_FAILURE_CONFLICT -> "conflit avec l'application installée (signature différente ?)"
      PackageInstaller.STATUS_FAILURE_INCOMPATIBLE -> "paquet incompatible avec ce téléphone"
      PackageInstaller.STATUS_FAILURE_INVALID -> "paquet invalide"
      PackageInstaller.STATUS_FAILURE_STORAGE -> "espace de stockage insuffisant"
      else -> "échec de l'installation (code $status)"
    }
    val extra = detail?.takeIf { it.isNotBlank() }?.let { " : $it" }.orEmpty()
    return "$reason$extra"
  }

  /** Après la mise à jour : Android a fermé l'application, la notification la rouvre. */
  fun notifyUpdated(context: Context) {
    val manager = context.getSystemService(NotificationManager::class.java) ?: return
    if (!manager.areNotificationsEnabled()) return
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
      manager.createNotificationChannel(
        NotificationChannel(CHANNEL, "Mises à jour", NotificationManager.IMPORTANCE_DEFAULT).apply {
          description = "PdfEditor mis à jour"
        },
      )
    }
    val open = PendingIntent.getActivity(
      context,
      0,
      Intent(context, MainActivity::class.java),
      PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
    )
    val builder = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
      Notification.Builder(context, CHANNEL)
    } else {
      @Suppress("DEPRECATION")
      Notification.Builder(context)
    }
    val notification = builder
      .setSmallIcon(R.drawable.ic_notification)
      .setContentTitle("PdfEditor mis à jour : version ${BuildConfig.VERSION_NAME}")
      .setContentText("Touchez pour rouvrir PdfEditor et voir les nouveautés.")
      .setContentIntent(open)
      .setAutoCancel(true)
      .build()
    manager.notify(NOTIFY_UPDATED, notification)
  }
}

/**
 * Faut-il renvoyer l'APK avec la confirmation d'Android ? Oui seulement
 * pour un refus d'une session sans confirmation (HyperOS de Xiaomi), APK
 * encore là. Une annulation par l'utilisateur passe forcément par la
 * confirmation, dont l'arrivée efface l'APK : elle n'est jamais relancée.
 */
fun shouldRetryWithConfirmation(status: Int, silent: Boolean, apkKept: Boolean): Boolean =
  status == PackageInstaller.STATUS_FAILURE_ABORTED && silent && apkKept

/** Résultat de l'installation, envoyé par Android (récepteur non exporté). */
class UpdateResultReceiver : BroadcastReceiver() {
  override fun onReceive(context: Context, intent: Intent) {
    val status = intent.getIntExtra(PackageInstaller.EXTRA_STATUS, PackageInstaller.STATUS_FAILURE)
    val silent = intent.getBooleanExtra(ApkInstaller.EXTRA_SILENT, false)
    val apk = intent.getStringExtra(ApkInstaller.EXTRA_APK)?.let(::File)?.takeIf { it.isFile }
    if (shouldRetryWithConfirmation(status, silent, apk != null)) {
      retry(context.applicationContext, apk!!)
      return
    }
    ApkInstaller.discard(context)
    when (status) {
      PackageInstaller.STATUS_PENDING_USER_ACTION -> {
        val confirm = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
          intent.getParcelableExtra(Intent.EXTRA_INTENT, Intent::class.java)
        } else {
          @Suppress("DEPRECATION")
          intent.getParcelableExtra(Intent.EXTRA_INTENT)
        }
        if (confirm == null) {
          ApkInstaller.report("failed", "confirmation d'Android introuvable")
          return
        }
        confirm.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        // Android interdit d'ouvrir un écran depuis l'arrière-plan : application
        // cachée, la confirmation s'ouvre au retour dans PdfEditor.
        val shown = ApkInstaller.visible && runCatching { context.startActivity(confirm) }.isSuccess
        if (!shown) ApkInstaller.pendingConfirm = confirm
        ApkInstaller.report("confirm")
      }
      // L'application est remplacée (Android la ferme) : rien d'autre à faire.
      PackageInstaller.STATUS_SUCCESS -> ApkInstaller.report("installed")
      else -> ApkInstaller.report(
        "failed",
        ApkInstaller.failure(status, intent.getStringExtra(PackageInstaller.EXTRA_STATUS_MESSAGE)),
      )
    }
  }

  /** Second essai avec confirmation, hors du fil principal (copie de l'APK). */
  private fun retry(context: Context, apk: File) {
    val pending = goAsync()
    Thread {
      try {
        ApkInstaller.install(context, apk, silent = false)
      } catch (error: Exception) {
        ApkInstaller.discard(context)
        ApkInstaller.report("failed", error.message ?: error.toString())
      } finally {
        pending.finish()
      }
    }.start()
  }
}

/** Application remplacée par une nouvelle version (envoyé par Android). */
class UpdatedReceiver : BroadcastReceiver() {
  override fun onReceive(context: Context, intent: Intent) {
    if (intent.action != Intent.ACTION_MY_PACKAGE_REPLACED || !ApkInstaller.supported) return
    runCatching { ApkInstaller.discard(context) }
    runCatching { ApkInstaller.notifyUpdated(context) }
  }
}
