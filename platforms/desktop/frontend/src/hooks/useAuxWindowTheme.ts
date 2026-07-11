import { useEffect, useRef, useState } from 'react';
import { settingsStore, STORE_KEY_CONFIG } from '../services/storageService';
import type { AppConfig } from '../types/config';
import { logger } from '../utils/logger';

type ThemePreference = NonNullable<AppConfig['theme']>;
export type ResolvedTheme = 'light' | 'dark';

function normalizeThemePreference(theme: AppConfig['theme']): ThemePreference {
    return theme ?? 'auto';
}

export function useAuxWindowTheme() {
    const [themePreference, setThemePreference] = useState<ThemePreference>('auto');
    const previousThemeRef = useRef<string | null | undefined>(undefined);

    useEffect(() => {
        previousThemeRef.current = document.documentElement.getAttribute('data-theme');

        return () => {
            if (previousThemeRef.current) {
                document.documentElement.setAttribute('data-theme', previousThemeRef.current);
                return;
            }

            document.documentElement.removeAttribute('data-theme');
        };
    }, []);

    useEffect(() => {
        let disposed = false;
        let unlisten: (() => void) | null = null;

        const applyConfig = (config: AppConfig | null | undefined) => {
            if (disposed) {
                return;
            }

            setThemePreference(normalizeThemePreference(config?.theme));
        };

        const setup = async () => {
            try {
                const storedConfig = await settingsStore.get<AppConfig>(STORE_KEY_CONFIG);
                applyConfig(storedConfig);
                unlisten = await settingsStore.onKeyChange<AppConfig>(STORE_KEY_CONFIG, applyConfig);
            } catch (error) {
                logger.warn('[useAuxWindowTheme] Failed to synchronize theme preference', {
                    error,
                });
            }
        };

        void setup();

        return () => {
            disposed = true;
            unlisten?.();
        };
    }, []);

    const [resolvedTheme, setResolvedTheme] = useState<ResolvedTheme>('light');

    useEffect(() => {
        const root = document.documentElement;

        const applyResolvedTheme = (nextTheme: ResolvedTheme) => {
            setResolvedTheme(nextTheme);
            root.setAttribute('data-theme', nextTheme);
        };

        if (themePreference === 'auto') {
            const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');

            applyResolvedTheme(mediaQuery.matches ? 'dark' : 'light');

            const handleChange = (event: MediaQueryListEvent) => {
                applyResolvedTheme(event.matches ? 'dark' : 'light');
            };

            mediaQuery.addEventListener('change', handleChange);
            return () => {
                mediaQuery.removeEventListener('change', handleChange);
            };
        }

        applyResolvedTheme(themePreference);
    }, [themePreference]);

    return resolvedTheme;
}
