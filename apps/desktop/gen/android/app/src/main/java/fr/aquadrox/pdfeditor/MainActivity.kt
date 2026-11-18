package fr.aquadrox.pdfeditor

import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import androidx.activity.enableEdgeToEdge
import java.io.File

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    rememberSharedDocument(intent)
  }

  // Application déjà ouverte (lancement unique) : le document reçu est noté,
  // l'interface le demande en revenant au premier plan.
  override fun onNewIntent(intent: Intent) {
    super.onNewIntent(intent)
    setIntent(intent)
    rememberSharedDocument(intent)
  }

  /**
   * « Ouvrir avec » (VIEW) ou « Partager » (SEND) un PDF : son adresse est
   * notée dans le cache ; Rust la lit (commande `shared_files`), copie le
   * document dans l'application et l'ouvre.
   */
  private fun rememberSharedDocument(intent: Intent?) {
    val uri: Uri = when (intent?.action) {
      Intent.ACTION_VIEW -> intent.data
      Intent.ACTION_SEND ->
        if (Build.VERSION.SDK_INT >= 33) {
          intent.getParcelableExtra(Intent.EXTRA_STREAM, Uri::class.java)
        } else {
          @Suppress("DEPRECATION")
          intent.getParcelableExtra(Intent.EXTRA_STREAM)
        }
      else -> null
    } ?: return
    File(cacheDir, "opened-document.txt").writeText(uri.toString())
  }
}
