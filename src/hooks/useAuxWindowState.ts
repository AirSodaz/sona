import { listen } from '@tauri-apps/api/event';
import { useEffect, useRef, useState } from 'react';
import { auxWindowStateService } from '../services/auxWindowStateService';

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
        let unlisten: (() => void) | null = null;

        const applyState = (nextState: T, source: 'snapshot' | 'event') => {
            if (disposed || nextState.revision < latestRevisionRef.current) {
                return;
            }

            latestRevisionRef.current = nextState.revision;
            setState(nextState);
            onStateAppliedRef.current?.(nextState, source);
        };

        const setup = async () => {
            const snapshotPromise = auxWindowStateService.get<T>(label);
            const unlistenFn = await listen<T>(eventName, (event) => {
                applyState(event.payload, 'event');
            });

            if (disposed) {
                unlistenFn();
                return;
            }

            unlisten = unlistenFn;

            const snapshot = await snapshotPromise;
            if (snapshot) {
                applyState(snapshot, 'snapshot');
            }
        };

        void setup();

        return () => {
            disposed = true;
            unlisten?.();
        };
    }, [defaultState.revision, eventName, label]);

    return state;
}
