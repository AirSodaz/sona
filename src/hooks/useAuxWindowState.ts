import { listen } from '@tauri-apps/api/event';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import { useEffect, useRef, useState } from 'react';
import { auxWindowStateService } from '../services/auxWindowStateService';
import { logger } from '../utils/logger';

const SNAPSHOT_POLL_INTERVAL_MS = 120;

interface UseAuxWindowStateOptions<T extends { revision: number }> {
    label: string;
    eventName: string;
    defaultState: T;
    onStateApplied?: (state: T, source: 'snapshot' | 'event') => void;
}

export function useAuxWindowState<T extends { revision: number }>(
    options: UseAuxWindowStateOptions<T>
) {
    const { defaultState, eventName, label, onStateApplied } = options;
    const [state, setState] = useState<T>(defaultState);
    const latestRevisionRef = useRef(defaultState.revision);
    const onStateAppliedRef = useRef(onStateApplied);

    useEffect(() => {
        onStateAppliedRef.current = onStateApplied;
    }, [onStateApplied]);

    useEffect(() => {
        let disposed = false;
        const unlistenCallbacks: Array<() => void> = [];
        let snapshotPollTimer: ReturnType<typeof setInterval> | null = null;
        let snapshotRefreshInFlight = false;

        const applyState = (nextState: T, source: 'snapshot' | 'event') => {
            if (disposed || nextState.revision < latestRevisionRef.current) {
                return;
            }

            latestRevisionRef.current = nextState.revision;
            setState(nextState);
            onStateAppliedRef.current?.(nextState, source);
        };

        const refreshSnapshot = async () => {
            if (disposed || snapshotRefreshInFlight) {
                return;
            }

            snapshotRefreshInFlight = true;
            try {
                const snapshot = await auxWindowStateService.get<T>(label);
                if (snapshot) {
                    applyState(snapshot, 'snapshot');
                }
            } finally {
                snapshotRefreshInFlight = false;
            }
        };

        const setup = async () => {
            const snapshotPromise = auxWindowStateService.get<T>(label);
            const handleEvent = (event: { payload: T }) => {
                applyState(event.payload, 'event');
            };

            try {
                const unlistenFn = await listen<T>(eventName, handleEvent);
                if (disposed) {
                    unlistenFn();
                } else {
                    unlistenCallbacks.push(unlistenFn);
                }
            } catch (error) {
                logger.warn('[useAuxWindowState] Failed to register app-level listener', {
                    label,
                    eventName,
                    error,
                });
            }

            try {
                const currentWindow = getCurrentWebviewWindow();
                const unlistenCurrentWindow = await currentWindow.listen<T>(eventName, handleEvent);
                if (disposed) {
                    unlistenCurrentWindow();
                } else {
                    unlistenCallbacks.push(unlistenCurrentWindow);
                }
            } catch (error) {
                logger.warn('[useAuxWindowState] Failed to register current-window listener', {
                    label,
                    eventName,
                    error,
                });
            }

            const snapshot = await snapshotPromise;
            if (snapshot) {
                applyState(snapshot, 'snapshot');
            }

            snapshotPollTimer = setInterval(() => {
                void refreshSnapshot();
            }, SNAPSHOT_POLL_INTERVAL_MS);
        };

        void setup();

        return () => {
            disposed = true;
            if (snapshotPollTimer) {
                clearInterval(snapshotPollTimer);
            }
            for (const unlisten of unlistenCallbacks) {
                unlisten();
            }
        };
    }, [defaultState.revision, eventName, label]);

    return state;
}
