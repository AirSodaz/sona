package com.sona.android.application.settings

import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class AppUpdateTest {
    @Test
    fun `newer remote version is available`() = runTest {
        val result = checker(release(versionCode = 12))(build(versionCode = 11))

        assertTrue(result is AppUpdateCheckResult.UpdateAvailable)
        assertEquals(12, result.latest.versionCode)
    }

    @Test
    fun `equal and older remote versions are up to date`() = runTest {
        for (remoteVersionCode in listOf(11, 10)) {
            val result = checker(release(versionCode = remoteVersionCode))(build(versionCode = 11))
            assertTrue(result is AppUpdateCheckResult.UpToDate)
        }
    }

    @Test(expected = IllegalArgumentException::class)
    fun `mismatched remote channel is rejected`() = runTest {
        checker(release(channel = AppUpdateChannel.NIGHTLY))(build())
    }

    @Test(expected = IllegalArgumentException::class)
    fun `invalid build version code is rejected`() {
        build(versionCode = 0)
    }

    @Test
    fun `selectAssetForAbis picks first matching device ABI`() {
        val arm = AppApkAsset(
            abi = "arm64-v8a",
            url = "https://github.com/AirSodaz/sona/releases/download/v1.2.4/app-arm64-v8a-debug.apk",
            sha256 = "a".repeat(64),
            sizeBytes = 1024L,
        )
        val x64 = AppApkAsset(
            abi = "x86_64",
            url = "https://github.com/AirSodaz/sona/releases/download/v1.2.4/app-x86_64-debug.apk",
            sha256 = "b".repeat(64),
            sizeBytes = 2048L,
        )
        val release = release().copy(assets = mapOf("arm64-v8a" to arm, "x86_64" to x64))

        assertEquals(arm, release.selectAssetForAbis(listOf("arm64-v8a", "armeabi-v7a")))
        assertEquals(x64, release.selectAssetForAbis(listOf("x86_64")))
        assertNull(release.selectAssetForAbis(listOf("armeabi-v7a")))
    }

    private fun checker(release: AppReleaseInfo) = CheckForAppUpdate(
        AppUpdatePort { _, _ -> release },
    )

    private fun build(versionCode: Int = 11) = AppBuildInfo(
        appName = "Sona",
        versionName = "1.2.3",
        versionCode = versionCode,
        channel = AppUpdateChannel.STABLE,
    )

    private fun release(
        versionCode: Int = 12,
        channel: AppUpdateChannel = AppUpdateChannel.STABLE,
    ) = AppReleaseInfo(
        versionName = "1.2.4",
        versionCode = versionCode,
        channel = channel,
        releasePageUrl = "https://github.com/AirSodaz/sona/releases/latest",
    )
}
