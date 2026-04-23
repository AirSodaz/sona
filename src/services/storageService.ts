import { LazyStore } from '@tauri-apps/plugin-store';

// Initialize the store. It will be saved as 'settings.json' in the app's appData/appConfig directory.
export const settingsStore = new LazyStore('settings.json');

// Helper keys
export const STORE_KEY_CONFIG = 'sona-config';
export const STORE_KEY_ONBOARDING = 'sona-onboarding';
export const STORE_KEY_ACTIVE_PROJECT = 'sona-active-project-id';
