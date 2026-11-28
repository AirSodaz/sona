pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
        maven {
            url = uri("sample-library/build/repo")
        }
    }
}

plugins {
    id("com.android.library") version "9.5.3" apply false
}

rootProject.name = "sona-uniffi-android-smoke"
include(":sample-library")
include(":consumer-library")
