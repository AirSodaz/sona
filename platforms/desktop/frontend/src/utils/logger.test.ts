import { describe, it, expect, vi, beforeEach } from 'vitest';
import { getLoggerLevel, logger, setLoggerLevel } from './logger';
import { debug as tauriDebug, info as tauriInfo, warn as tauriWarn, error as tauriError, trace as tauriTrace } from '@tauri-apps/plugin-log';

vi.mock('@tauri-apps/plugin-log', () => ({
  trace: vi.fn().mockResolvedValue(undefined),
  debug: vi.fn().mockResolvedValue(undefined),
  info: vi.fn().mockResolvedValue(undefined),
  warn: vi.fn().mockResolvedValue(undefined),
  error: vi.fn().mockResolvedValue(undefined),
}));

describe('logger utility', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    setLoggerLevel('info');
  });

  it('defaults to info and skips debug messages', async () => {
    await logger.debug('Test message', { data: 123 });

    expect(getLoggerLevel()).toBe('info');
    expect(tauriDebug).not.toHaveBeenCalled();
  });

  it('formats info messages without args correctly', async () => {
    await logger.info('Test info');
    expect(tauriInfo).toHaveBeenCalledWith('Test info');
  });

  it('allows debug messages when configured for debug', async () => {
    setLoggerLevel('debug');

    await logger.debug('Test message', { data: 123 });

    expect(tauriDebug).toHaveBeenCalledWith('Test message [{"data":123}]');
  });

  it('allows trace messages only when configured for trace', async () => {
    await logger.trace('Trace skipped');
    expect(tauriTrace).not.toHaveBeenCalled();

    setLoggerLevel('trace');
    await logger.trace('Trace written');

    expect(tauriTrace).toHaveBeenCalledWith('Trace written');
  });

  it('filters lower-priority messages when configured for warn', async () => {
    setLoggerLevel('warn');

    await logger.debug('Debug skipped');
    await logger.info('Info skipped');
    await logger.warn('Warn written');
    await logger.error('Error written');

    expect(tauriDebug).not.toHaveBeenCalled();
    expect(tauriInfo).not.toHaveBeenCalled();
    expect(tauriWarn).toHaveBeenCalledWith('Warn written');
    expect(tauriError).toHaveBeenCalledWith('Error written');
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
