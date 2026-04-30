import {
    clearAuxWindowState,
    getAuxWindowState,
    setAuxWindowState,
} from './tauri/system';

export const auxWindowStateService = {
    async set<T>(label: string, payload: T) {
        await setAuxWindowState(label, payload);
    },

    async get<T>(label: string) {
        return await getAuxWindowState<T>(label);
    },

    async clear(label: string) {
        await clearAuxWindowState(label);
    },
};
