import org.gradle.api.JavaVersion

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.plugin.compose")
}

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
        applicationId = "com.sona.android"
        minSdk = 23
        targetSdk = 37
        versionCode = 1
        versionName = "0.8.0"
    }

    buildFeatures {
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
    implementation("androidx.compose.material:material-icons-extended")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.material3:material3-adaptive-navigation-suite")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.11.0")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.11.0")
    implementation("androidx.lifecycle:lifecycle-viewmodel-ktx:2.11.0")
    implementation("androidx.navigation:navigation-compose:2.9.8")
    implementation("com.google.android.material:material:1.14.0")

    debugImplementation("androidx.compose.ui:ui-tooling")
}
