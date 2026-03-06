import { describe, it, expect, vi, beforeEach } from 'vitest';
import { logger } from './logger';
import { debug as tauriDebug, info as tauriInfo, warn as tauriWarn, error as tauriError } from '@tauri-apps/plugin-log';

vi.mock('@tauri-apps/plugin-log', () => ({
  debug: vi.fn().mockResolvedValue(undefined),
  info: vi.fn().mockResolvedValue(undefined),
  warn: vi.fn().mockResolvedValue(undefined),
  error: vi.fn().mockResolvedValue(undefined),
}));

describe('logger utility', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('formats debug messages correctly', async () => {
    await logger.debug('Test message', { data: 123 });
    expect(tauriDebug).toHaveBeenCalledWith('Test message [{"data":123}]');
  });

  it('formats info messages without args correctly', async () => {
    await logger.info('Test info');
    expect(tauriInfo).toHaveBeenCalledWith('Test info');
  });

  it('formats warn messages correctly', async () => {
    await logger.warn('Test warn', 'warning', 42);
    expect(tauriWarn).toHaveBeenCalledWith('Test warn ["warning",42]');
  });

  it('formats error messages correctly', async () => {
    await logger.error('Test error', new Error('test'));
    expect(tauriError).toHaveBeenCalled();
  });
});
