import org.gradle.api.JavaVersion
import org.gradle.api.publish.maven.MavenPublication
import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    id("com.android.library")
    id("maven-publish")
}

group = "com.sona"
version = "0.8.0"

android {
    namespace = "com.sona.uniffi.sample"
    compileSdk = 36

    defaultConfig {
        minSdk = 23
    }

    publishing {
        singleVariant("debug")
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

kotlin {
    compilerOptions {
        jvmTarget.set(JvmTarget.JVM_17)
    }
}

apply(from = "../../sona-uniffi-bindings.gradle.kts")

afterEvaluate {
    publishing {
        publications {
            create<MavenPublication>("debug") {
                from(components["debug"])
                groupId = "com.sona"
                artifactId = "sona-uniffi-bindings"
                version = project.version.toString()

                pom {
                    name.set("Sona UniFFI Android Bindings")
                    description.set("Generated Kotlin and JNI bindings for the Sona Rust core.")
                }
            }
        }
        repositories {
            maven {
                name = "SonaAndroidSample"
                url = uri(layout.buildDirectory.dir("repo"))
            }
        }
    }
}
