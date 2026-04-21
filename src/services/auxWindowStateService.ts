import { invoke } from '@tauri-apps/api/core';

export const auxWindowStateService = {
    async set<T>(label: string, payload: T) {
        await invoke('set_aux_window_state', { label, payload });
    },

    async get<T>(label: string) {
        return await invoke<T | null>('get_aux_window_state', { label });
    },

    async clear(label: string) {
        await invoke('clear_aux_window_state', { label });
    },
};
