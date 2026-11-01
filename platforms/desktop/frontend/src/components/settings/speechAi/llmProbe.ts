import type { LlmProvider } from '../../../types/transcript';

const STORAGE_KEY = 'sona.speechAi.llmProbe';
export const PROBE_STALE_MS = 10 * 60 * 1000;

export interface LlmProbeRecord {
  provider: LlmProvider;
  model: string;
  ok: boolean;
  at: number;
  latencyMs?: number;
  error?: string;
}

export function readLlmProbe(): LlmProbeRecord | null {
  if (typeof localStorage === 'undefined') return null;
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as LlmProbeRecord;
    if (!parsed || typeof parsed.at !== 'number' || typeof parsed.provider !== 'string') {
      return null;
    }
    return parsed;
  } catch {
    return null;
  }
}

export function writeLlmProbe(record: LlmProbeRecord): void {
  if (typeof localStorage === 'undefined') return;
  localStorage.setItem(STORAGE_KEY, JSON.stringify(record));
}

export function probeIsFresh(record: LlmProbeRecord | null, now = Date.now()): boolean {
  if (!record?.ok) return false;
  return now - record.at < PROBE_STALE_MS;
}

type Translate = (key: string, options?: Record<string, unknown>) => string;

export function formatCheckedAgo(at: number, t: Translate, now = Date.now()): string {
  const minutes = Math.max(0, Math.round((now - at) / 60000));
  if (minutes < 1) return t('settings.speech_ai.checked_just_now');
  if (minutes === 1) return t('settings.speech_ai.checked_one_minute');
  if (minutes < 60) return t('settings.speech_ai.checked_minutes', { count: minutes });
  const hours = Math.round(minutes / 60);
  if (hours === 1) return t('settings.speech_ai.checked_one_hour');
  return t('settings.speech_ai.checked_hours', { count: hours });
}
