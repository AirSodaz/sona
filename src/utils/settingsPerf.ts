import { logger } from './logger';

export const SETTINGS_PERF_STORAGE_KEY = 'sona-settings-perf-enabled';

export interface SettingsPerfEvent {
  name: string;
  atMs: number;
  sinceFirstMs: number;
  sinceOpenMs: number | null;
  detail?: Record<string, unknown>;
}

interface SettingsPerfDevApi {
  enable: () => void;
  disable: () => void;
  snapshot: () => SettingsPerfEvent[];
  clear: () => void;
}

declare global {
  interface Window {
    sonaSettingsPerf?: SettingsPerfDevApi;
  }
}

const OPEN_EVENT_PREFIX = 'settings.open.';

let events: SettingsPerfEvent[] = [];
let currentOpenStartMs: number | null = null;

function getNowMs(): number {
  if (typeof performance !== 'undefined' && typeof performance.now === 'function') {
    return performance.now();
  }

  return Date.now();
}

function roundMs(value: number): number {
  return Math.round(value * 100) / 100;
}

function canUseLocalStorage(): boolean {
  return typeof localStorage !== 'undefined';
}

export function isSettingsPerfEnabled(): boolean {
  if (!canUseLocalStorage()) {
    return false;
  }

  try {
    const value = localStorage.getItem(SETTINGS_PERF_STORAGE_KEY);
    return value === 'true' || value === '1';
  } catch {
    return false;
  }
}

export function enableSettingsPerf(): void {
  if (!canUseLocalStorage()) {
    return;
  }

  try {
    localStorage.setItem(SETTINGS_PERF_STORAGE_KEY, 'true');
  } catch {
    // Ignore storage failures; perf logging is diagnostic-only.
  }
}

export function disableSettingsPerf(): void {
  if (!canUseLocalStorage()) {
    return;
  }

  try {
    localStorage.removeItem(SETTINGS_PERF_STORAGE_KEY);
  } catch {
    // Ignore storage failures; perf logging is diagnostic-only.
  }
}

export function clearSettingsPerf(): void {
  events = [];
  currentOpenStartMs = null;
}

export function snapshotSettingsPerf(): SettingsPerfEvent[] {
  return events.map((event) => ({
    ...event,
    detail: event.detail ? { ...event.detail } : undefined,
  }));
}

export function getSettingsPerfErrorDetail(error: unknown): Record<string, unknown> {
  if (error instanceof Error) {
    return {
      name: error.name,
      message: error.message,
    };
  }

  return {
    message: String(error),
  };
}

export function markSettingsPerf(name: string, detail?: Record<string, unknown>): void {
  if (!isSettingsPerfEnabled()) {
    return;
  }

  const atMs = roundMs(getNowMs());
  if (name.startsWith(OPEN_EVENT_PREFIX)) {
    currentOpenStartMs = atMs;
  }

  const firstAtMs = events[0]?.atMs ?? atMs;
  const event: SettingsPerfEvent = {
    name,
    atMs,
    sinceFirstMs: roundMs(atMs - firstAtMs),
    sinceOpenMs: currentOpenStartMs === null ? null : roundMs(atMs - currentOpenStartMs),
    ...(detail ? { detail } : {}),
  };

  events.push(event);
  void logger.info(`[SettingsPerf] ${name}`, event);
}

function installSettingsPerfDevTools(): void {
  if (!import.meta.env.DEV || typeof window === 'undefined') {
    return;
  }

  window.sonaSettingsPerf = {
    enable: enableSettingsPerf,
    disable: disableSettingsPerf,
    snapshot: snapshotSettingsPerf,
    clear: clearSettingsPerf,
  };
}

installSettingsPerfDevTools();
