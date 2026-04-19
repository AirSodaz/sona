import { useEffect } from 'react';
import { useUIConfig } from '../stores/configStore';

export function useFontEffect() {
    const { font } = useUIConfig();

    useEffect(() => {
        const currentFont = font || 'system';
        const root = document.documentElement;

        const setFontVars = (fontFamily: string) => {
            root.style.setProperty('--font-sans', fontFamily);
            root.style.setProperty('--font-serif', fontFamily);
            root.style.setProperty('--font-mono', fontFamily);
        };

        const removeFontVars = () => {
            root.style.removeProperty('--font-sans');
            root.style.removeProperty('--font-serif');
            root.style.removeProperty('--font-mono');
        };

        switch (currentFont) {
            case 'serif':
                setFontVars('Merriweather, serif');
                break;
            case 'sans':
                setFontVars('Inter, sans-serif');
                break;
            case 'mono':
                setFontVars('JetBrains Mono, monospace');
                break;
            case 'arial':
                setFontVars('Arial, sans-serif');
                break;
            case 'georgia':
                setFontVars('Georgia, serif');
                break;
            case 'system':
            default:
                removeFontVars();
                break;
        }
    }, [font]);
}
