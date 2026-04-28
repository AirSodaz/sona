import { useEffect, useState } from 'react';
import { logger } from '../utils/logger';
import { useThemeEffect } from './useThemeEffect';
import { useFontEffect } from './useFontEffect';
import { useConfigPersistence } from './useConfigPersistence';
import { useTraySyncEffect } from './useTraySyncEffect';
import { hydrateAppStartupState } from '../services/startup/hydration';
import { startAppRuntimeServices } from '../services/startup/runtime';

/**
 * Hook to handle application initialization.
 *
 * - Hydrates the config/onboarding/project state needed for first paint.
 * - Starts non-blocking runtime services after the app shell becomes ready.
 * - Initializes decoupled side-effects (theme, font, persistence, tray).
 */
export function useAppInitialization() {
    const [isLoaded, setIsLoaded] = useState(false);

    // Initialize decoupled side-effects
    useThemeEffect();
    useFontEffect();
    useConfigPersistence(isLoaded);
    useTraySyncEffect(isLoaded);

    // Initialize config and onboarding state
    useEffect(() => {
        let cancelled = false;

        async function initialize() {
            try {
                await hydrateAppStartupState();
            } catch (e) {
                logger.error('[Startup] Failed to initialize app state:', e);
            } finally {
                if (!cancelled) {
                    setIsLoaded(true);
                    void startAppRuntimeServices().catch((error) => {
                        logger.error('[Startup] Failed to start background runtime services:', error);
                    });
                }
            }
        }

        void initialize();

        return () => {
            cancelled = true;
        };
    }, []); // Run once on mount

    return { isLoaded };
}
