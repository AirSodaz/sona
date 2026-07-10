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
Android `main` source set, and stages ABI-specific `libsona_uniffi_bind.so`
files under generated `jniLibs`.

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

This AAR stage supports the online streaming path. Packaging the local Sherpa
Android native libraries is out of scope, so this artifact does not claim
local Sherpa ASR support on Android.

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
`jni/arm64-v8a/libsona_uniffi_bind.so`, compiled `uniffi.sona_uniffi_bind`
classes, and the sample `SonaUniffiSmoke` class. The local Maven publication is
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
