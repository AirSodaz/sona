package com.sona.android.application.settings

import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
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

    private fun checker(release: AppReleaseInfo) = CheckForAppUpdate(
        AppUpdatePort { release },
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
