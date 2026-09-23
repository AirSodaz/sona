package com.sona.android.adapters.uniffi.recording

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import java.io.InputStream

data class AndroidModelSource(
    val id: String,
    val name: String,
    val modelType: String,
    val quantization: String = "int8",
    val size: String,
    val languages: List<String> = emptyList(),
    val languageMode: String = "auto",
    val modes: List<String> = emptyList(),
    val isRecommended: Boolean = false,
    val description: String? = null,
)

class AndroidModelSourceRepository(
    private val resourceStreamProvider: () -> InputStream? = {
        AndroidModelSourceRepository::class.java.classLoader
            ?.getResourceAsStream("android-model-sources.json")
    },
) {
    private val json = Json {
        ignoreUnknownKeys = true
        isLenient = true
    }

    fun loadSources(): List<AndroidModelSource> {
        val stream = try {
            resourceStreamProvider()
        } catch (_: Throwable) {
            null
        }

        val parsed = if (stream != null) {
            try {
                val text = stream.bufferedReader().use { it.readText() }
                parseSourcesJson(text)
            } catch (_: Throwable) {
                FALLBACK_SOURCES
            }
        } else {
            FALLBACK_SOURCES
        }

        return parsed.filter { it.quantization.equals("int8", ignoreCase = true) }
    }

    private fun parseSourcesJson(text: String): List<AndroidModelSource> {
        val root = json.parseToJsonElement(text).jsonObject
        val modelsArray = root["models"]?.jsonArray ?: return FALLBACK_SOURCES
        return modelsArray.mapNotNull { element ->
            val obj = element.jsonObject
            val id = obj["id"]?.jsonPrimitive?.content ?: return@mapNotNull null
            val name = obj["name"]?.jsonPrimitive?.content ?: id
            val modelType = obj["modelType"]?.jsonPrimitive?.content ?: ""
            val quantization = obj["quantization"]?.jsonPrimitive?.content ?: "int8"
            val size = obj["size"]?.jsonPrimitive?.content ?: ""
            val languages = obj["languages"]?.jsonArray?.mapNotNull { it.jsonPrimitive.content } ?: emptyList()
            val languageMode = obj["languageMode"]?.jsonPrimitive?.content ?: "auto"
            val modes = obj["modes"]?.jsonArray?.mapNotNull { it.jsonPrimitive.content } ?: emptyList()
            val isRecommended = obj["isRecommended"]?.jsonPrimitive?.booleanOrNull ?: false
            val description = obj["description"]?.jsonPrimitive?.content

            AndroidModelSource(
                id = id,
                name = name,
                modelType = modelType,
                quantization = quantization,
                size = size,
                languages = languages,
                languageMode = languageMode,
                modes = modes,
                isRecommended = isRecommended,
                description = description,
            )
        }
    }

    companion object {
        val FALLBACK_SOURCES: List<AndroidModelSource> = listOf(
            AndroidModelSource(
                id = "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17",
                name = "SenseVoice (Int8)",
                modelType = "sensevoice",
                quantization = "int8",
                size = "~240 MB",
                languages = listOf("zh", "en", "ja", "ko", "yue"),
                languageMode = "auto",
                modes = listOf("streaming", "batch"),
                isRecommended = true,
                description = "Multi-language high-accuracy recognition, recommended for mobile devices",
            ),
            AndroidModelSource(
                id = "sherpa-onnx-moonshine-tiny-en-quantized-2026-02-27",
                name = "Moonshine Tiny EN (Int8)",
                modelType = "moonshine",
                quantization = "int8",
                size = "~44 MB",
                languages = listOf("en"),
                languageMode = "fixed",
                modes = listOf("streaming", "batch"),
                isRecommended = false,
            ),
            AndroidModelSource(
                id = "sherpa-onnx-moonshine-base-zh-quantized-2026-02-27",
                name = "Moonshine Base ZH (Int8)",
                modelType = "moonshine",
                quantization = "int8",
                size = "~141 MB",
                languages = listOf("zh"),
                languageMode = "fixed",
                modes = listOf("streaming", "batch"),
                isRecommended = false,
            ),
            AndroidModelSource(
                id = "sherpa-onnx-moonshine-base-en-quantized-2026-02-27",
                name = "Moonshine Base EN (Int8)",
                modelType = "moonshine",
                quantization = "int8",
                size = "~141 MB",
                languages = listOf("en"),
                languageMode = "fixed",
                modes = listOf("streaming", "batch"),
                isRecommended = false,
            ),
            AndroidModelSource(
                id = "sherpa-onnx-streaming-paraformer-trilingual-zh-cantonese-en-int8",
                name = "Paraformer Trilingual (Int8)",
                modelType = "paraformer",
                quantization = "int8",
                size = "~238 MB",
                languages = listOf("zh", "yue", "en"),
                languageMode = "auto",
                modes = listOf("streaming"),
                isRecommended = false,
            ),
            AndroidModelSource(
                id = "sherpa-onnx-dolphin-small-ctc-multi-lang-int8-2025-04-02",
                name = "Dolphin Small Multi-Lang (Int8)",
                modelType = "dolphin",
                quantization = "int8",
                size = "~250 MB",
                languages = listOf("zh", "en", "ja", "ko", "de", "es", "fr", "ru"),
                languageMode = "auto",
                modes = listOf("streaming", "batch"),
                isRecommended = false,
            ),
            AndroidModelSource(
                id = "sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8",
                name = "Parakeet TDT 0.6B (Int8)",
                modelType = "parakeet-tdt",
                quantization = "int8",
                size = "~670 MB",
                languages = listOf("en"),
                languageMode = "fixed",
                modes = listOf("streaming", "batch"),
                isRecommended = false,
            ),
            AndroidModelSource(
                id = "sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25",
                name = "Qwen3 ASR 0.6B (Int8)",
                modelType = "qwen3-asr",
                quantization = "int8",
                size = "~943 MB",
                languages = listOf("zh", "en", "yue"),
                languageMode = "auto",
                modes = listOf("streaming", "batch"),
                isRecommended = false,
            ),
            AndroidModelSource(
                id = "sherpa-onnx-funasr-nano-int8-2025-12-30",
                name = "FunASR Nano (Int8)",
                modelType = "funasr-nano",
                quantization = "int8",
                size = "~994 MB",
                languages = listOf("zh", "en"),
                languageMode = "auto",
                modes = listOf("streaming"),
                isRecommended = false,
            ),
        )
    }
}
