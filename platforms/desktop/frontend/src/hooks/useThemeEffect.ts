import { useEffect } from 'react';
import { useUIConfig } from '../stores/configStore';

export function useThemeEffect() {
    const { theme } = useUIConfig();

    useEffect(() => {
        const currentTheme = theme || 'auto';
        const root = document.documentElement;

        const applyTheme = (targetTheme: string) => {
            switch (targetTheme) {
                case 'dark':
                case 'light':
                    root.setAttribute('data-theme', targetTheme);
                    break;
                default:
                    root.removeAttribute('data-theme');
                    break;
            }
        };

        if (currentTheme === 'auto') {
            const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');

            // Initial check
            applyTheme(mediaQuery.matches ? 'dark' : 'light');

            // Listen for changes
            const handleChange = (e: MediaQueryListEvent) => {
                applyTheme(e.matches ? 'dark' : 'light');
            };

            mediaQuery.addEventListener('change', handleChange);
            return () => mediaQuery.removeEventListener('change', handleChange);
        } else {
            applyTheme(currentTheme);
        }
    }, [theme]);
}
