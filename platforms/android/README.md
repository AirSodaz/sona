# Android UniFFI Bindings

Apply `platforms/android/sona-uniffi-bindings.gradle.kts` from an Android
library module to generate Kotlin bindings from `sona-uniffi-bind`.

```kotlin
apply(from = "../../platforms/android/sona-uniffi-bindings.gradle.kts")
```

Set `SONA_REPO_ROOT` when the Android module is not two directories below the
Sona repository root. It can be a Gradle property or an environment variable.
The sample consumer uses `gradle.properties`:

```properties
SONA_REPO_ROOT=../../../..
```

The Gradle script registers `generateSonaUniffiKotlin` and
`buildSonaUniffiAndroidLibraries`, adds the generated Kotlin directory to the
Android `main` source set, and stages ABI-specific `libsona_uniffi_bind.so`,
`libsherpa-onnx-c-api.so`, and `libonnxruntime.so` files under generated
`jniLibs`.

The generated streaming surface is typed. Implement
`FfiAsrStreamingObserver`, then pass it to `createOnlineAsrStreamingSession`:

```kotlin
import uniffi.sona_uniffi_bind.FfiAsrInferenceMetric
import uniffi.sona_uniffi_bind.FfiAsrModelLoadMetric
import uniffi.sona_uniffi_bind.FfiAsrStreamingObserver
import uniffi.sona_uniffi_bind.FfiAsrStreamingSession
import uniffi.sona_uniffi_bind.FfiAsrTranscriptUpdateEvent
import uniffi.sona_uniffi_bind.createOnlineAsrStreamingSession

fun createStreamingSession(requestJson: String): FfiAsrStreamingSession {
    val observer: FfiAsrStreamingObserver = object : FfiAsrStreamingObserver {
        override fun onTranscriptUpdate(event: FfiAsrTranscriptUpdateEvent) = Unit
        override fun onModelLoad(metric: FfiAsrModelLoadMetric) = Unit
        override fun onLiveInference(metric: FfiAsrInferenceMetric) = Unit
    }

    return createOnlineAsrStreamingSession(
        instanceId = "android-live-1",
        requestJson = requestJson,
        observer = observer,
    )
}
```

The session's `start`, audio-feed, and `stop` methods are suspending Kotlin
methods and must run from a coroutine. Close the session when it is no longer
needed.

## Recovery snapshots

The generated recovery surface exposes these JSON functions:

- `loadRecoverySnapshotJson(appDataDir)`
- `saveRecoverySnapshotJson(appDataDir, itemsJson)`
- `persistRecoveryQueueSnapshotJson(appDataDir, queueItemsJson, resolvedIds)`

The caller supplies an application data directory for every operation.
`itemsJson` and `queueItemsJson` are JSON arrays. Each result uses the
canonical camelCase version-1 recovery snapshot format. These operations
perform filesystem I/O and can fail with `SonaCoreBindingException`.

This AAR includes the native local Sherpa runtime needed for later Android
local ASR integration. The generated Kotlin surface remains online-only until
a local streaming session factory is explicitly added to the UniFFI API.

The Android build downloads the locked sherpa-onnx 1.13.4 archive into
`target/android-sherpa`. For offline builds, set
`SONA_SHERPA_ONNX_ANDROID_ARCHIVE` to a local copy of the locked archive. The
local copy must match the SHA-256 recorded in
`platforms/android/packaging/sherpa-onnx-sources.json`.

Run the local smoke check without requiring a Gradle install:

```powershell
pnpm run verify:android-uniffi
```

Run the Gradle-backed smoke check with a repo-managed Gradle distribution:

```powershell
pnpm run verify:android-uniffi:gradle
```

The managed runner downloads Gradle `9.6.1` into `target/managed-gradle`,
checks the distribution SHA-256, and runs the sample project from there. It
does not commit a Gradle wrapper jar to the repository.
The smoke check assembles the sample debug AAR, runs
`:sample-library:publishDebugPublicationToSonaAndroidSampleRepository`, and
verifies both outputs. The AAR must contain
`jni/arm64-v8a/libsona_uniffi_bind.so`,
`jni/arm64-v8a/libsherpa-onnx-c-api.so`,
`jni/arm64-v8a/libonnxruntime.so`, compiled `uniffi.sona_uniffi_bind` classes,
and the sample `SonaUniffiSmoke` class. The local Maven publication is
written under `platforms/android/sample-consumer/sample-library/build/repo` as
`com.sona:sona-uniffi-bindings:0.8.0`, with POM dependencies for JNA and
Kotlin coroutines. The verifier also checks the Gradle Module Metadata runtime
variant so Gradle consumers resolve the same AAR and runtime dependencies.

On networks that cannot reach the Gradle distribution CDN, pre-download the
official `gradle-9.6.1-bin.zip` and point the runner at it:

```powershell
$env:SONA_GRADLE_DISTRIBUTION_ZIP="D:\downloads\gradle-9.6.1-bin.zip"
pnpm run verify:android-uniffi:gradle
```

`platforms/android/sample-consumer` is a minimal AGP 9 smoke project. Its
`:sample-library` module applies the same script and imports the generated
`uniffi.sona_uniffi_bind` Kotlin API. Use it as the smoke-test shape for app
modules that need the Sona core bindings.

The sample also includes `:consumer-library`, which does not apply the UniFFI
generation script. It consumes the locally published AAR through:

```kotlin
implementation("com.sona:sona-uniffi-bindings:0.8.0")
```

This guards the shape a normal Android app or library would use after the
binding artifact has been published.

Use `SONA_ANDROID_ABIS` to limit native builds, for example:

```powershell
$env:SONA_ANDROID_ABIS="arm64-v8a,x86_64"
```

Use `SONA_ANDROID_MIN_SDK` to override the Android linker API level. The
default is `23`.

## Android client

The native Compose client lives under `platforms/android/client` and keeps its
host boundary explicit:

- `:application` owns platform-neutral models, ports, and use cases.
- `:adapters:android` owns Android framework integration for microphone capture,
  WAV persistence, input monitoring, secure credential storage, clocks, and IDs.
- `:adapters:uniffi` is the only module that imports generated UniFFI APIs.
- `:app` owns Android lifecycle, adaptive Compose navigation, and dependency
  composition.

Kotlin sources stay feature-first inside those module boundaries. The app keeps
dependency construction under `composition`, navigation under `navigation`, and
screen-specific state and UI under `feature/<name>`. The application recording
slice keeps its models and policies together while separating credential,
microphone, streaming, history, and system ports into cohesive files. Android
framework and UniFFI implementations remain grouped by adapter capability.

The recording adapter captures 16 kHz mono PCM16 from the platform
`VOICE_RECOGNITION` source. Accepted audio is written to a checkpointing WAV
file before it is offered to streaming ASR as 640-byte frames. The non-blocking
100-frame queue holds two seconds of audio. If that queue overflows, the
adapter stops further ASR delivery and reports the overflow while continuing to
write the complete WAV recording. Android 7.0 and later use recording callbacks
for input monitoring, Android 10 and later report client silencing, and Android
11 and later mark the capture as privacy-sensitive.

Streaming credentials are encrypted with a non-exportable Android Keystore
AES-256-GCM key and provider-generated IVs. DataStore persists only the versioned
encrypted envelope under `noBackupFilesDir`; it never stores plaintext and the
file is excluded from device backup. Settings consumers receive only configured
status plus save/clear capabilities. Plaintext resolution is available only to
the live-recording coordinator when a session starts.

The client compiles and targets Android API 37 with min SDK 23. Install
`platforms;android-37.0` through `sdkmanager`, then run the complete client
unit-test and APK gate from the repository root:

```powershell
pnpm run verify:android-client
```

The default client verification builds and validates two independent debug APKs:

- `platforms/android/client/app/build/outputs/apk/debug/app-arm64-v8a-debug.apk`
  for Android phones and tablets.
- `platforms/android/client/app/build/outputs/apk/debug/app-x86_64-debug.apk`
  for x86_64 emulators and devices.

Each APK contains only its matching Sona UniFFI, sherpa-onnx, and ONNX Runtime
native libraries. Set `SONA_ANDROID_ABIS` to one of the supported values when a
single-ABI local build is sufficient.

The verification command runs application and Android-adapter unit tests,
assembles the adapter instrumentation-test APK, lints both the adapter and app,
then assembles and validates both client APKs. Instrumentation assembly proves
API compatibility but does not execute device APIs. Real microphone, recording
callback, privacy-sensitive capture, and Android Keystore behavior must run on
API 23 and API 37 emulators as the device QA gate.

GitHub Actions uses `.github/workflows/android-client.yml` as the reusable
Android build entry point. Stable and nightly workflows call it with both ABIs,
publish the two APKs as separate workflow artifacts, and attach both files to
tagged or nightly GitHub releases. A manually dispatched Android workflow uses
the same build and verification path.

These CI outputs are currently debug-signed preview packages. They are
installable for testing, but they are not production-signed and a package from
one CI run may need to be uninstalled before installing a package signed by a
different run's generated debug key.
