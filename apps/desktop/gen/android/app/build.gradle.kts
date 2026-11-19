import java.util.Properties
import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("rust")
}

val tauriProperties = Properties().apply {
    val propFile = file("tauri.properties")
    if (propFile.exists()) {
        propFile.inputStream().use { load(it) }
    }
}

/** Version de l'application : `version` de `[workspace.package]` du Cargo.toml racine. */
val appVersion: String = run {
    val cargo = rootProject.file("../../../../Cargo.toml").readText()
    val section = cargo.substringAfter("[workspace.package]").substringBefore("\n[")
    Regex("""(?m)^version\s*=\s*"(\d+\.\d+\.\d+)"""").find(section)?.groupValues?.get(1)
        ?: throw GradleException("version introuvable dans [workspace.package] du Cargo.toml racine")
}

/** « 0.0.30 » → 30 : même formule que Tauri (majeure × 1 000 000 + mineure × 1 000 + correctif). */
val appVersionCode: Int = appVersion.split('.').map { it.toInt() }.let { (major, minor, patch) ->
    require(minor < 1000 && patch < 1000) { "version $appVersion : mineure et correctif < 1000" }
    major * 1_000_000 + minor * 1_000 + patch
}

android {
    compileSdk = 37
    namespace = "fr.aquadrox.pdfeditor"
    defaultConfig {
        manifestPlaceholders["usesCleartextTraffic"] = "false"
        applicationId = "fr.aquadrox.pdfeditor"
        minSdk = 24
        targetSdk = 37
        // Version du Cargo.toml racine (tauri.conf.json n'en a pas : Tauri
        // n'écrirait que 1.0). Les mises à jour (A-04) en dépendent : Android
        // refuse un versionCode qui ne monte pas, l'application relit versionName.
        versionCode = tauriProperties.getProperty("tauri.android.versionCode")?.toInt() ?: appVersionCode
        versionName = tauriProperties.getProperty("tauri.android.versionName") ?: appVersion
    }
    // Clé de signature des releases (A-04) : fournie par la CI (secrets
    // GitHub), jamais dans le dépôt. Sans elle, l'APK de release n'est pas
    // signé. Android n'accepte une mise à jour que signée avec la même clé.
    val keystore = System.getenv("ANDROID_KEYSTORE_FILE")
    signingConfigs {
        if (keystore != null) {
            create("release") {
                storeFile = file(keystore)
                storePassword = System.getenv("ANDROID_KEYSTORE_PASSWORD")
                keyAlias = System.getenv("ANDROID_KEY_ALIAS")
                keyPassword = System.getenv("ANDROID_KEY_PASSWORD")
            }
        }
    }
    buildTypes {
        getByName("debug") {
            // Version de test (PR) : s'installe à côté de la version publiée,
            // sans mise à jour automatique (autre clé de signature).
            manifestPlaceholders["usesCleartextTraffic"] = "true"
            isDebuggable = true
            isJniDebuggable = true
            isMinifyEnabled = false
            packaging {
                jniLibs.keepDebugSymbols.add("*/arm64-v8a/*.so")
                jniLibs.keepDebugSymbols.add("*/armeabi-v7a/*.so")
                jniLibs.keepDebugSymbols.add("*/x86/*.so")
                jniLibs.keepDebugSymbols.add("*/x86_64/*.so")
            }
        }
        getByName("release") {
            if (keystore != null) signingConfig = signingConfigs.getByName("release")
            optimization {
               enable = true
            }
            proguardFiles(
                *fileTree(".") {
                  include("**/*.pro")
                  exclude("build/**")
                }.files.toTypedArray()
            )
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_1_8
        targetCompatibility = JavaVersion.VERSION_1_8
    }
    buildFeatures {
        buildConfig = true
    }
}

kotlin {
    compilerOptions {
        jvmTarget = JvmTarget.JVM_1_8
    }
}

rust {
    rootDirRel = "../../../"
}

dependencies {
    implementation("androidx.webkit:webkit:1.14.0")
    implementation("androidx.appcompat:appcompat:1.7.1")
    implementation("androidx.activity:activity-ktx:1.10.1")
    implementation("com.google.android.material:material:1.12.0")
    implementation("androidx.lifecycle:lifecycle-process:2.10.0")
    testImplementation("junit:junit:4.13.2")
    androidTestImplementation("androidx.test.ext:junit:1.1.4")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.5.0")
}

apply(from = file("tauri.build.gradle.kts"))
