package com.sona.android.adapters.uniffi.settings

import com.sona.android.application.recording.AsrModelSelection
import com.sona.android.application.recording.BatchCredentialSettingsPort
import com.sona.android.application.recording.LocalAsrDeviceCapabilitiesPort
import com.sona.android.application.recording.LocalAsrModel
import com.sona.android.application.recording.OnlineAsrProvider
import com.sona.android.application.recording.RecognitionSettingsPort
import com.sona.android.application.settings.AppLogPort
import com.sona.android.application.settings.DiagnosticsModelCheck
import com.sona.android.application.settings.DiagnosticsPort
import com.sona.android.application.settings.DiagnosticsSummary
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.withContext
import uniffi.sona_uniffi_bind.FfiAsrRuntimeMetricsSnapshotV1
import uniffi.sona_uniffi_bind.FfiDiagnosticsConfigV1
import uniffi.sona_uniffi_bind.FfiDiagnosticsDeviceProbeV1
import uniffi.sona_uniffi_bind.FfiDiagnosticsInputV1
import uniffi.sona_uniffi_bind.FfiDiagnosticsModelRulesV1
import uniffi.sona_uniffi_bind.FfiDiagnosticsModelSummaryV1
import uniffi.sona_uniffi_bind.FfiDiagnosticsPathStatusesV1
import uniffi.sona_uniffi_bind.FfiDiagnosticsSelectedModelsV1
import uniffi.sona_uniffi_bind.FfiRuntimeEnvironmentStatusV1
import uniffi.sona_uniffi_bind.FfiRuntimePathKind
import uniffi.sona_uniffi_bind.FfiRuntimePathStatus
import uniffi.sona_uniffi_bind.FfiVoiceTypingReadinessV1
import uniffi.sona_uniffi_bind.loadDiagnosticsSnapshotV1

class UniffiDiagnosticsAdapter(
    private val appDataDir: String,
    private val logDirPath: String,
    private val recognitionSettings: RecognitionSettingsPort,
    private val batchCredentials: BatchCredentialSettingsPort,
    private val deviceCapabilities: LocalAsrDeviceCapabilitiesPort,
    private val appLogs: AppLogPort,
) : DiagnosticsPort {
    override suspend fun loadSummary(
        microphonePermissionGranted: Boolean,
        notificationPermissionGranted: Boolean,
    ): DiagnosticsSummary = withContext(Dispatchers.IO) {
        val recognition = recognitionSettings.load()
        val capabilities = deviceCapabilities.detect()
        val installedById = recognition.installedModels.associateBy { it.id }

        val liveModel = (recognition.liveSelection as? AsrModelSelection.Local)?.modelId?.let(installedById::get)
        val batchModel = (recognition.batchSelection as? AsrModelSelection.Local)?.modelId?.let(installedById::get)

        val input = FfiDiagnosticsInputV1(
            config = FfiDiagnosticsConfigV1(
                streamingModelPath = liveModel?.config?.modelPath.orEmpty(),
                batchModelPath = batchModel?.config?.modelPath.orEmpty(),
                vadModelPath = (liveModel?.config?.vadModel ?: batchModel?.config?.vadModel).orEmpty(),
                punctuationModelPath = (liveModel?.config?.punctuationModel ?: batchModel?.config?.punctuationModel).orEmpty(),
                microphoneId = "mobile-default",
            ),
            selectedModels = FfiDiagnosticsSelectedModelsV1(
                live = liveModel?.let { FfiDiagnosticsModelSummaryV1(it.id, it.displayName) },
                batch = batchModel?.let { FfiDiagnosticsModelSummaryV1(it.id, it.displayName) },
            ),
            modelRules = FfiDiagnosticsModelRulesV1(live = null, batch = null),
            pathStatuses = FfiDiagnosticsPathStatusesV1(
                liveModel = null,
                batchModel = null,
                vad = null,
                punctuation = null,
            ),
            permissionState = if (microphonePermissionGranted) "granted" else "denied",
            microphoneProbe = FfiDiagnosticsDeviceProbeV1(
                options = emptyList(),
                available = microphonePermissionGranted,
                errorMessage = if (microphonePermissionGranted) null else "microphone_permission_denied",
            ),
            systemAudioProbe = FfiDiagnosticsDeviceProbeV1(
                options = emptyList(),
                available = false,
                errorMessage = "unsupported",
            ),
            voiceTypingReadiness = FfiVoiceTypingReadinessV1(
                state = if (microphonePermissionGranted) "ready" else "blocked",
                lastErrorMessage = null,
            ),
            runtimeEnvironment = FfiRuntimeEnvironmentStatusV1(
                ffmpegPath = "mobile://ffmpeg",
                ffmpegExists = false,
                logDirPath = logDirPath,
            ),
            asrRuntimeMetrics = FfiAsrRuntimeMetricsSnapshotV1(
                modelLoad = null,
                liveInference = null,
                batchInference = null,
            ),
            onboardingReady = true,
            punctuationRequired = false,
        )

        val snapshot = loadDiagnosticsSnapshotV1(appDataDir, input)
        val configuredProviders = batchCredentials.configuration.first().configuredProviders
        val liveCheck = checkSlot(
            "live",
            recognition.liveSelection,
            installedById,
            configuredProviders,
            snapshot.pathStatuses.liveModel.isPresent(),
        )
        val batchCheck = checkSlot(
            "batch",
            recognition.batchSelection,
            installedById,
            configuredProviders,
            snapshot.pathStatuses.batchModel.isPresent(),
        )

        val overallStatus = if (microphonePermissionGranted && liveCheck.ready && batchCheck.ready) {
            "ready"
        } else {
            "attention"
        }

        DiagnosticsSummary(
            overallStatus = overallStatus,
            deviceAbi = capabilities.primaryAbi,
            deviceTier = capabilities.tier.name.lowercase(),
            cpuCores = capabilities.cpuCores,
            totalMemoryBytes = capabilities.totalMemoryBytes,
            availableStorageBytes = capabilities.availableStorageBytes,
            microphonePermissionGranted = microphonePermissionGranted,
            notificationPermissionGranted = notificationPermissionGranted,
            liveModelCheck = liveCheck,
            batchModelCheck = batchCheck,
            logFileSizeBytes = appLogs.logSizeBytes(),
            scannedAt = snapshot.scannedAt,
        )
    }

    private fun FfiRuntimePathStatus?.isPresent(): Boolean? = this?.let {
        (it.kind == FfiRuntimePathKind.FILE || it.kind == FfiRuntimePathKind.DIRECTORY) &&
            it.error == null
    }

    private fun checkSlot(
        slotName: String,
        selection: AsrModelSelection?,
        installedById: Map<String, LocalAsrModel>,
        configuredProviders: Set<OnlineAsrProvider>,
        pathExists: Boolean?,
    ): DiagnosticsModelCheck = when (selection) {
        null -> DiagnosticsModelCheck(slotName, "None", false, "not_selected")
        is AsrModelSelection.Online -> {
            val hasKey = selection.provider in configuredProviders
            DiagnosticsModelCheck(
                slotName = slotName,
                label = selection.provider.name,
                ready = hasKey,
                detail = if (hasKey) "credential_configured" else "credential_missing",
            )
        }
        is AsrModelSelection.Local -> {
            val model = installedById[selection.modelId]
            val ready = model != null && (pathExists != false)
            DiagnosticsModelCheck(
                slotName = slotName,
                label = model?.displayName ?: selection.modelId,
                ready = ready,
                detail = if (ready) "installed" else "model_missing",
            )
        }
    }
}
