pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

fun resolveRustlsPlatformVerifierMavenDir(rootDir: java.io.File): java.io.File {
    val override = providers.gradleProperty("RUSTLS_PLATFORM_VERIFIER_MAVEN_DIR")
        .orElse(providers.environmentVariable("RUSTLS_PLATFORM_VERIFIER_MAVEN_DIR"))
        .orNull
    if (!override.isNullOrBlank()) {
        val dir = java.io.File(override)
        require(dir.isDirectory) {
            "RUSTLS_PLATFORM_VERIFIER_MAVEN_DIR is set to '$override', but directory does not exist"
        }
        return dir
    }

    val repoRoot = rootDir.parentFile.parentFile
    val output: String
    try {
        val process = ProcessBuilder("cargo", "metadata", "--format-version", "1")
            .directory(repoRoot)
            .redirectErrorStream(true)
            .start()
        output = process.inputStream.bufferedReader().readText()
        val exitCode = process.waitFor()
        require(exitCode == 0) {
            "Failed to run cargo metadata to locate rustls-platform-verifier-android:\n$output"
        }
    } catch (e: Exception) {
        throw IllegalStateException(
            "Could not run 'cargo metadata' to locate rustls-platform-verifier AAR. " +
                "Ensure cargo is installed and in PATH, or specify the local maven repository directory via " +
                "RUSTLS_PLATFORM_VERIFIER_MAVEN_DIR environment variable or Gradle property. Cause: ${e.message}",
            e,
        )
    }
    val match = Regex(""""manifest_path"\s*:\s*"([^"]*rustls-platform-verifier-android[^"]*Cargo\.toml)"""")
        .find(output)
        ?: error("Could not find package 'rustls-platform-verifier-android' in cargo metadata output")
    val manifestPath = java.io.File(match.groupValues[1].replace("\\\\", java.io.File.separator))
    val mavenDir = java.io.File(manifestPath.parentFile, "maven")
    require(mavenDir.isDirectory) {
        "Expected rustls-platform-verifier maven repository at '$mavenDir', but directory does not exist"
    }
    return mavenDir
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
        val mavenDir = resolveRustlsPlatformVerifierMavenDir(rootProject.projectDir)
        exclusiveContent {
            forRepository {
                maven {
                    url = uri(mavenDir)
                    metadataSources {
                        artifact()
                        mavenPom()
                    }
                }
            }
            filter {
                includeGroup("rustls")
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
