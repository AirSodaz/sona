import org.gradle.api.JavaVersion

plugins {
    id("com.android.library")
}

android {
    namespace = "com.sona.android.adapters.uniffi"
    compileSdk = 37

    defaultConfig {
        minSdk = 23
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

dependencies {
    implementation(project(":application"))
}

apply(from = "../../../sona-uniffi-bindings.gradle.kts")
