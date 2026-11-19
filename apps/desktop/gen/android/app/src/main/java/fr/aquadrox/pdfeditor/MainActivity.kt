package fr.aquadrox.pdfeditor

import android.content.Intent
import android.content.res.Configuration
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.view.View
import androidx.activity.enableEdgeToEdge
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import java.io.File

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    keepClearOfSystemBars()
    rememberSharedDocument(intent)
  }

  /**
   * Bord à bord (imposé par Android 15) : la page web s'étendrait sous la
   * barre d'état, la barre de navigation et le clavier. L'interface reste
   * dans la zone libre : marges égales aux barres du système, et au clavier
   * quand il est ouvert (le champ en cours de saisie reste visible). Les
   * marges prennent la couleur des barres de l'interface (jeton `--surface`,
   * clair ou sombre comme le système).
   */
  private fun keepClearOfSystemBars() {
    val content = findViewById<View>(android.R.id.content)
    val night = (resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK) ==
      Configuration.UI_MODE_NIGHT_YES
    content.setBackgroundColor(if (night) SURFACE_DARK else SURFACE_LIGHT)
    ViewCompat.setOnApplyWindowInsetsListener(content) { view, insets ->
      val bars = insets.getInsets(
        WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout(),
      )
      val keyboard = insets.getInsets(WindowInsetsCompat.Type.ime())
      view.setPadding(bars.left, bars.top, bars.right, maxOf(bars.bottom, keyboard.bottom))
      // Déjà appliquées : la page ne les recompte pas (env(safe-area-inset-*) à 0).
      WindowInsetsCompat.CONSUMED
    }
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

  private companion object {
    /** `--surface` de ui/styles/tokens.css, thèmes clair et sombre. */
    val SURFACE_LIGHT: Int = 0xFFF6F7F8.toInt()
    val SURFACE_DARK: Int = 0xFF22262B.toInt()
  }
}
