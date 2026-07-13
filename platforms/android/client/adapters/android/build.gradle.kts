import org.gradle.api.JavaVersion

plugins {
    id("com.android.library")
}

android {
    namespace = "com.sona.android.adapters.android"
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
