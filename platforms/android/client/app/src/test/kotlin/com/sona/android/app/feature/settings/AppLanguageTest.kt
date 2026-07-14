package com.sona.android.app.feature.settings

import org.junit.Assert.assertEquals
import org.junit.Test

class AppLanguageTest {
    @Test
    fun emptyLanguageTagsFollowTheSystem() {
        assertEquals(AppLanguage.SYSTEM, AppLanguage.fromLanguageTags(""))
        assertEquals(AppLanguage.SYSTEM, AppLanguage.fromLanguageTags("   "))
    }

    @Test
    fun canonicalLanguageTagsMapToSupportedLanguages() {
        val cases = mapOf(
            "en" to AppLanguage.ENGLISH,
            "zh-Hans" to AppLanguage.SIMPLIFIED_CHINESE,
            "zh-Hant" to AppLanguage.TRADITIONAL_CHINESE,
            "ja" to AppLanguage.JAPANESE,
            "ko" to AppLanguage.KOREAN,
        )

        cases.forEach { (tag, expected) ->
            assertEquals(tag, expected, AppLanguage.fromLanguageTags(tag))
        }
    }

    @Test
    fun regionalLanguageTagsMapToTheMatchingSupportedLanguage() {
        val cases = mapOf(
            "en-US" to AppLanguage.ENGLISH,
            "zh-CN" to AppLanguage.SIMPLIFIED_CHINESE,
            "zh_SG" to AppLanguage.SIMPLIFIED_CHINESE,
            "zh-TW" to AppLanguage.TRADITIONAL_CHINESE,
            "zh-HK" to AppLanguage.TRADITIONAL_CHINESE,
            "zh-MO" to AppLanguage.TRADITIONAL_CHINESE,
            "ja-JP" to AppLanguage.JAPANESE,
            "ko-KR" to AppLanguage.KOREAN,
        )

        cases.forEach { (tag, expected) ->
            assertEquals(tag, expected, AppLanguage.fromLanguageTags(tag))
        }
    }

    @Test
    fun onlyThePrimaryApplicationLocaleControlsTheSelection() {
        assertEquals(
            AppLanguage.TRADITIONAL_CHINESE,
            AppLanguage.fromLanguageTags("zh-Hant,en"),
        )
    }

    @Test
    fun unsupportedLanguageTagsFallBackToTheSystem() {
        assertEquals(AppLanguage.SYSTEM, AppLanguage.fromLanguageTags("fr"))
        assertEquals(AppLanguage.SYSTEM, AppLanguage.fromLanguageTags("not-a-locale"))
    }
}
