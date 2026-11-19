package fr.aquadrox.pdfeditor

import android.content.pm.PackageInstaller
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** Reprise de l'installation refusée par Xiaomi (HyperOS), comme VoiceToText 0.33.2. */
class ApkInstallerTest {
  @Test
  fun refusedSilentInstallIsRetriedWithConfirmation() {
    // « INSTALL_FAILED_ABORTED: Permission denied » au premier appui.
    assertTrue(shouldRetryWithConfirmation(PackageInstaller.STATUS_FAILURE_ABORTED, silent = true, apkKept = true))
  }

  @Test
  fun cancellationByTheUserIsNeverRetried() {
    // Annulation dans la fenêtre d'Android : session avec confirmation.
    assertFalse(shouldRetryWithConfirmation(PackageInstaller.STATUS_FAILURE_ABORTED, silent = false, apkKept = true))
    // La confirmation arrivée efface l'APK : rien à renvoyer.
    assertFalse(shouldRetryWithConfirmation(PackageInstaller.STATUS_FAILURE_ABORTED, silent = true, apkKept = false))
  }

  @Test
  fun otherOutcomesAreNotRetried() {
    for (status in listOf(
      PackageInstaller.STATUS_SUCCESS,
      PackageInstaller.STATUS_PENDING_USER_ACTION,
      PackageInstaller.STATUS_FAILURE_CONFLICT,
      PackageInstaller.STATUS_FAILURE_STORAGE,
    )) {
      assertFalse(shouldRetryWithConfirmation(status, silent = true, apkKept = true))
    }
  }
}
