import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import LanguageDetector from 'i18next-browser-languagedetector';
import { resolveDetectedAppLanguage, SUPPORTED_APP_LANGUAGES } from './constants/appLanguages';

/**
 * Configuration for internationalization using i18next.
 *
 * Sets up resources, language detection, and fallback language.
 */

import en from './locales/en.json';
import ja from './locales/ja.json';
import ko from './locales/ko.json';
import zh from './locales/zh.json';
import zhTW from './locales/zh-TW.json';

const resources = {
    en: {
        translation: en
    },
    ja: {
        translation: ja
    },
    ko: {
        translation: ko
    },
    zh: {
        translation: zh
    },
    'zh-TW': {
        translation: zhTW
    }
};

i18n
    .use(LanguageDetector)
    .use(initReactI18next)
    .init({
        resources,
        supportedLngs: [...SUPPORTED_APP_LANGUAGES],
        fallbackLng: 'en',
        interpolation: {
            escapeValue: false // react already safes from xss
        },
        detection: {
            order: ['navigator', 'htmlTag', 'path', 'subdomain'],
            caches: [],
            convertDetectedLanguage: resolveDetectedAppLanguage
        }
    });

export default i18n;
