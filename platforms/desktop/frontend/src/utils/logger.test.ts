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

  it('formats and forwards plain messages at every level', async () => {
    const cases = [
      { level: 'trace', run: () => logger.trace('Test trace'), plugin: tauriTrace, expected: 'Test trace' },
      { level: 'debug', run: () => logger.debug('Test debug', { data: 123 }), plugin: tauriDebug, expected: 'Test debug [{"data":123}]' },
      { level: 'info', run: () => logger.info('Test info'), plugin: tauriInfo, expected: 'Test info' },
      { level: 'warn', run: () => logger.warn('Test warn', 'warning', 42), plugin: tauriWarn, expected: 'Test warn ["warning",42]' },
      { level: 'error', run: () => logger.error('Test error', new Error('test')), plugin: tauriError, expected: expect.stringContaining('Test error') },
    ] as const;

    for (const { level, run, plugin, expected } of cases) {
      setLoggerLevel(level);
      await run();
      expect(plugin).toHaveBeenCalledWith(expected);
    }
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
});
