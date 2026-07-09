import com.android.build.api.dsl.LibraryExtension
import java.io.File
import org.gradle.api.tasks.Exec
import org.jetbrains.kotlin.gradle.tasks.KotlinCompile

val sonaRepoRoot = providers.gradleProperty("SONA_REPO_ROOT")
    .orElse(providers.environmentVariable("SONA_REPO_ROOT"))
    .map { file(it) }
    .orElse(layout.projectDirectory.dir("../..").asFile)
val sonaAndroidAbis = providers.environmentVariable("SONA_ANDROID_ABIS")
val sonaAndroidMinSdk = providers.environmentVariable("SONA_ANDROID_MIN_SDK")
    .orElse("23")
val generatedKotlinDir = layout.buildDirectory.dir("generated/source/uniffi/main/kotlin")
val generatedJniLibsDir = layout.buildDirectory.dir("generated/jniLibs/main")

val buildSonaUniffiAndroidLibraries = tasks.register<Exec>("buildSonaUniffiAndroidLibraries") {
    val repoRoot = sonaRepoRoot.get()
    val outDir = generatedJniLibsDir.get().asFile
    val androidAbis = sonaAndroidAbis.orNull
    val androidMinSdk = sonaAndroidMinSdk.get()

    workingDir = repoRoot
    inputs.file(File(repoRoot, "Cargo.toml"))
    inputs.file(File(repoRoot, "Cargo.lock"))
    inputs.file(File(repoRoot, "adapters/uniffi_bind/Cargo.toml"))
    inputs.dir(File(repoRoot, "adapters/uniffi_bind/src"))
    inputs.dir(File(repoRoot, "core/src"))
    inputs.file(File(repoRoot, "scripts/build-uniffi-android-libs.js"))
    inputs.file(File(repoRoot, "adapters/runtime_fs/Cargo.toml"))
    inputs.dir(File(repoRoot, "adapters/runtime_fs/src"))
    inputs.property("sonaAndroidAbis", androidAbis ?: "")
    inputs.property("sonaAndroidMinSdk", androidMinSdk)
    outputs.dir(outDir)
    val command = mutableListOf(
        "node",
        File(repoRoot, "scripts/build-uniffi-android-libs.js").path,
        "--out-dir",
        outDir.path,
        "--min-sdk",
        androidMinSdk,
    )
    if (!androidAbis.isNullOrBlank()) {
        command.addAll(listOf("--abis", androidAbis))
    }
    commandLine(command)
}

val generateSonaUniffiKotlin = tasks.register<Exec>("generateSonaUniffiKotlin") {
    val repoRoot = sonaRepoRoot.get()
    val outDir = generatedKotlinDir.get().asFile

    workingDir = repoRoot
    inputs.file(File(repoRoot, "Cargo.toml"))
    inputs.file(File(repoRoot, "Cargo.lock"))
    inputs.file(File(repoRoot, "adapters/uniffi_bind/Cargo.toml"))
    inputs.dir(File(repoRoot, "adapters/uniffi_bind/src"))
    inputs.dir(File(repoRoot, "core/src"))
    inputs.file(File(repoRoot, "scripts/generate-uniffi-kotlin.js"))
    inputs.file(File(repoRoot, "tools/uniffi_bindgen/Cargo.toml"))
    inputs.dir(File(repoRoot, "tools/uniffi_bindgen/src"))
    outputs.dir(outDir)
    commandLine(
        "node",
        File(repoRoot, "scripts/generate-uniffi-kotlin.js").path,
        "--out-dir",
        outDir.path,
    )
}

plugins.withId("com.android.library") {
    extensions.configure<LibraryExtension>("android") {
        sourceSets.getByName("main") {
            java.directories.add(generatedKotlinDir.get().asFile.path)
            jniLibs.directories.add(generatedJniLibsDir.get().asFile.path)
        }
    }

    dependencies.add("implementation", "net.java.dev.jna:jna:5.12.0@aar")
    dependencies.add("implementation", "org.jetbrains.kotlinx:kotlinx-coroutines-core:1.6.4")

    tasks.matching { it.name == "preBuild" }
        .configureEach {
            dependsOn(buildSonaUniffiAndroidLibraries)
        }
    tasks.matching { it.name.startsWith("extract") && it.name.endsWith("Annotations") }
        .configureEach {
            dependsOn(generateSonaUniffiKotlin)
        }
    tasks.withType<KotlinCompile>()
        .configureEach {
            if (!name.endsWith("AndroidTestKotlin") && !name.endsWith("UnitTestKotlin")) {
                source(generatedKotlinDir)
            }
            dependsOn(buildSonaUniffiAndroidLibraries)
            dependsOn(generateSonaUniffiKotlin)
        }
}
