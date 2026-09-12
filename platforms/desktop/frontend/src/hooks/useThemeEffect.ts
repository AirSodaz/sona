import { useEffect } from 'react';
import { setWindowTheme } from '../services/tauri/app';
import { useUIConfig } from '../stores/configStore';
import type { AppThemePreference, ResolvedAppTheme } from '../types/config';
import { logger } from '../utils/logger';

/**
 * Resolves a theme preference to a concrete theme.
 *
 * `auto` follows the OS color scheme; every other value is already concrete.
 */
function resolveTheme(preference: AppThemePreference): ResolvedAppTheme {
  if (preference === 'auto') {
    return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
  }

  return preference;
}

/**
 * Applies the resolved theme to the document and the native window frame.
 *
 * The document drives all in-app styling through the `data-theme` attribute.
 * The native frame is drawn by the OS and cannot read our CSS custom
 * properties, so the resolved value is also pushed across the IPC boundary
 * to keep the title bar in sync with the application theme.
 */
export function useThemeEffect() {
  const { theme } = useUIConfig();

  useEffect(() => {
    const preference = theme || 'auto';
    const root = document.documentElement;

    const applyTheme = (targetTheme: ResolvedAppTheme) => {
      root.setAttribute('data-theme', targetTheme);
      // Fire-and-forget: a failure here only leaves the frame on its
      // previous color, which must never break theme application itself.
      setWindowTheme(targetTheme).catch((error) =>
        logger.warn('[useThemeEffect] Failed to sync native window theme', {
          theme: targetTheme,
          error,
        })
      );
    };

    if (preference === 'auto') {
      const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');

      applyTheme(resolveTheme('auto'));

      const handleChange = () => {
        applyTheme(resolveTheme('auto'));
      };

      mediaQuery.addEventListener('change', handleChange);
      return () => mediaQuery.removeEventListener('change', handleChange);
    }

    applyTheme(preference);
  }, [theme]);
}
