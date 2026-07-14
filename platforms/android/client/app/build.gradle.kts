import org.gradle.api.JavaVersion

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.plugin.compose")
}

val maximumAndroidVersionCode = 2_100_000_000L

fun environmentValue(name: String): String =
    providers.environmentVariable(name).orNull?.trim().orEmpty()

val sonaAndroidChannel = environmentValue("SONA_ANDROID_CHANNEL").ifEmpty { "stable" }
require(sonaAndroidChannel == "stable" || sonaAndroidChannel == "nightly") {
    "SONA_ANDROID_CHANNEL must be stable or nightly"
}

val suppliedAndroidVersionName = environmentValue("SONA_ANDROID_VERSION_NAME")
val suppliedAndroidVersionCode = environmentValue("SONA_ANDROID_VERSION_CODE")
if (sonaAndroidChannel == "nightly") {
    require(suppliedAndroidVersionName.isNotEmpty()) {
        "SONA_ANDROID_VERSION_NAME is required for nightly builds"
    }
    require(suppliedAndroidVersionCode.isNotEmpty()) {
        "SONA_ANDROID_VERSION_CODE is required for nightly builds"
    }
}

val sonaAndroidVersionName = suppliedAndroidVersionName.ifEmpty { "0.8.0" }
val sonaAndroidVersionCodeValue = suppliedAndroidVersionCode.ifEmpty { "1" }
val parsedAndroidVersionCode = sonaAndroidVersionCodeValue.toLongOrNull()
require(
    sonaAndroidVersionCodeValue.matches(Regex("[0-9]+")) &&
        parsedAndroidVersionCode != null &&
        parsedAndroidVersionCode in 1L..maximumAndroidVersionCode,
) {
    "SONA_ANDROID_VERSION_CODE must be an integer from 1 to $maximumAndroidVersionCode"
}
val sonaAndroidVersionCode = checkNotNull(parsedAndroidVersionCode).toInt()
val sonaAndroidApplicationId = if (sonaAndroidChannel == "nightly") {
    "com.sona.android.nightly"
} else {
    "com.sona.android"
}
val sonaAndroidAppName = if (sonaAndroidChannel == "nightly") "Sona Nightly" else "Sona"

val sonaAndroidAbis = providers.environmentVariable("SONA_ANDROID_ABIS")
    .orElse("arm64-v8a,x86_64")
    .get()
    .split(',')
    .map(String::trim)
    .filter(String::isNotEmpty)
    .distinct()

require(sonaAndroidAbis.isNotEmpty()) {
    "SONA_ANDROID_ABIS must select at least one Android ABI"
}

android {
    namespace = "com.sona.android.app"
    compileSdk = 37

    defaultConfig {
        applicationId = sonaAndroidApplicationId
        minSdk = 23
        targetSdk = 37
        versionCode = sonaAndroidVersionCode
        versionName = sonaAndroidVersionName
        manifestPlaceholders["sonaAppName"] = sonaAndroidAppName
        buildConfigField("String", "APP_NAME", "\"$sonaAndroidAppName\"")
    }

    buildFeatures {
        buildConfig = true
        compose = true
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    packaging {
        resources {
            excludes += "/META-INF/{AL2.0,LGPL2.1}"
        }
    }

    lint {
        // The dynamic split list is verified against both generated APKs below.
        disable += "ChromeOsAbiSupport"
        enable += setOf("MissingTranslation", "ExtraTranslation")
        warningsAsErrors = true
    }

    splits {
        abi {
            isEnable = true
            reset()
            include(*sonaAndroidAbis.toTypedArray())
            isUniversalApk = false
        }
    }
}

dependencies {
    implementation(project(":application"))
    implementation(project(":adapters:android"))
    implementation(project(":adapters:uniffi"))

    implementation(platform("androidx.compose:compose-bom:2026.06.01"))
    implementation("androidx.activity:activity-compose:1.13.0")
    implementation("androidx.appcompat:appcompat:1.7.1")
    implementation("androidx.compose.material:material-icons-extended")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.material3:material3-adaptive-navigation-suite")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.11.0")
    implementation("androidx.lifecycle:lifecycle-process:2.11.0")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.11.0")
    implementation("androidx.lifecycle:lifecycle-viewmodel-ktx:2.11.0")
    implementation("androidx.navigation:navigation-compose:2.9.8")
    implementation("com.google.android.material:material:1.14.0")

    debugImplementation("androidx.compose.ui:ui-tooling")

    testImplementation("junit:junit:4.13.2")
    testImplementation("org.jetbrains.kotlinx:kotlinx-coroutines-test:1.11.0")
}
