package com.sona.android.app.feature.settings

import com.sona.android.app.MainDispatcherRule
import com.sona.android.application.settings.AppBuildInfo
import com.sona.android.application.settings.AppReleaseInfo
import com.sona.android.application.settings.AppUpdateChannel
import com.sona.android.application.settings.AppUpdatePort
import com.sona.android.application.settings.CheckForAppUpdate
import java.net.SocketTimeoutException
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

class AboutSettingsViewModelTest {
    @get:Rule
    val mainDispatcherRule = MainDispatcherRule()

    @Test
    fun `automatic check runs once and publishes an available update`() = runTest {
        val updates = FakeUpdatePort(release(versionCode = 12))
        val viewModel = viewModel(updates)

        viewModel.checkIfNeeded()
        viewModel.checkIfNeeded()
        mainDispatcherRule.dispatcher.scheduler.runCurrent()

        assertEquals(1, updates.calls)
        assertTrue(viewModel.state.value.updateStatus is AboutUpdateStatus.UpdateAvailable)
    }

    @Test
    fun `manual retry checks again after an error`() = runTest {
        val updates = FakeUpdatePort(release()).apply {
            failure = SocketTimeoutException("network detail")
        }
        val viewModel = viewModel(updates)

        viewModel.checkIfNeeded()
        mainDispatcherRule.dispatcher.scheduler.runCurrent()
        assertTrue(viewModel.state.value.updateStatus is AboutUpdateStatus.Error)

        updates.failure = null
        viewModel.checkForUpdates()
        mainDispatcherRule.dispatcher.scheduler.runCurrent()

        assertEquals(2, updates.calls)
        assertTrue(viewModel.state.value.updateStatus is AboutUpdateStatus.UpdateAvailable)
    }

    @Test
    fun `concurrent checks are suppressed`() = runTest {
        val updates = FakeUpdatePort(release()).apply { gate = CompletableDeferred() }
        val viewModel = viewModel(updates)

        viewModel.checkForUpdates()
        mainDispatcherRule.dispatcher.scheduler.runCurrent()
        viewModel.checkForUpdates()
        mainDispatcherRule.dispatcher.scheduler.runCurrent()

        assertEquals(1, updates.calls)
        assertTrue(viewModel.state.value.updateStatus is AboutUpdateStatus.Checking)
        updates.gate?.complete(Unit)
        mainDispatcherRule.dispatcher.scheduler.runCurrent()
    }

    @Test
    fun `equal version is reported as current`() = runTest {
        val updates = FakeUpdatePort(release(versionCode = 11))
        val viewModel = viewModel(updates)

        viewModel.checkForUpdates()
        mainDispatcherRule.dispatcher.scheduler.runCurrent()

        assertTrue(viewModel.state.value.updateStatus is AboutUpdateStatus.UpToDate)
    }

    private fun viewModel(updates: AppUpdatePort) = AboutSettingsViewModel(
        build = build(),
        checkForAppUpdate = CheckForAppUpdate(updates),
    )

    private class FakeUpdatePort(
        private val release: AppReleaseInfo,
    ) : AppUpdatePort {
        var calls = 0
        var failure: Throwable? = null
        var gate: CompletableDeferred<Unit>? = null

        override suspend fun latestRelease(channel: AppUpdateChannel): AppReleaseInfo {
            calls += 1
            gate?.await()
            failure?.let { throw it }
            return release
        }
    }

    companion object {
        private fun build() = AppBuildInfo(
            appName = "Sona",
            versionName = "1.2.3",
            versionCode = 11,
            channel = AppUpdateChannel.STABLE,
        )

        private fun release(versionCode: Int = 12) = AppReleaseInfo(
            versionName = "1.2.4",
            versionCode = versionCode,
            channel = AppUpdateChannel.STABLE,
            releasePageUrl = "https://github.com/AirSodaz/sona/releases/latest",
        )
    }
}
