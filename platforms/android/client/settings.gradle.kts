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
        val override = providers.gradleProperty("RUSTLS_PLATFORM_VERIFIER_MAVEN_DIR")
            .orElse(providers.environmentVariable("RUSTLS_PLATFORM_VERIFIER_MAVEN_DIR"))
            .orNull
        exclusiveContent {
            if (!override.isNullOrBlank()) {
                val dir = java.io.File(override)
                if (dir.isDirectory) {
                    forRepository {
                        maven {
                            url = uri(dir)
                            metadataSources {
                                artifact()
                                mavenPom()
                            }
                        }
                    }
                }
            }
            forRepository {
                maven {
                    url = uri("https://raw.githubusercontent.com/rustls/rustls-platform-verifier/maven-archive/android-release-support/maven/")
                }
            }
            filter {
                includeGroup("org.rustls")
            }
        }
    }
}

plugins {
    id("com.android.application") version "9.2.1" apply false
    id("com.android.library") version "9.2.1" apply false
    id("org.jetbrains.kotlin.jvm") version "2.2.10" apply false
    id("org.jetbrains.kotlin.plugin.compose") version "2.2.10" apply false
}

rootProject.name = "sona-android-client"
include(":application")
include(":adapters:android")
include(":adapters:uniffi")
include(":app")
