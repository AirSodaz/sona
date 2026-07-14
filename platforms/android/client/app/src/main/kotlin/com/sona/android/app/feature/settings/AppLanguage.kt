package com.sona.android.app.feature.settings

import androidx.annotation.StringRes
import com.sona.android.app.R
import java.util.Locale

internal enum class AppLanguage(
    val languageTag: String,
    @StringRes val labelRes: Int,
) {
    SYSTEM("", R.string.language_follow_system),
    ENGLISH("en", R.string.language_english),
    SIMPLIFIED_CHINESE("zh-Hans", R.string.language_simplified_chinese),
    TRADITIONAL_CHINESE("zh-Hant", R.string.language_traditional_chinese),
    JAPANESE("ja", R.string.language_japanese),
    KOREAN("ko", R.string.language_korean),
    ;

    companion object {
        private val traditionalChineseRegions = setOf("TW", "HK", "MO")

        fun fromLanguageTags(languageTags: String): AppLanguage {
            val firstLanguageTag = languageTags
                .substringBefore(',')
                .trim()
                .replace('_', '-')
            if (firstLanguageTag.isEmpty()) return SYSTEM

            val locale = Locale.forLanguageTag(firstLanguageTag)
            return when (locale.language.lowercase(Locale.ROOT)) {
                "en" -> ENGLISH
                "zh" -> if (
                    locale.script.equals("Hant", ignoreCase = true) ||
                    locale.country.uppercase(Locale.ROOT) in traditionalChineseRegions
                ) {
                    TRADITIONAL_CHINESE
                } else {
                    SIMPLIFIED_CHINESE
                }
                "ja" -> JAPANESE
                "ko" -> KOREAN
                else -> SYSTEM
            }
        }
    }
}
