import {
  debug as tauriDebug,
  error as tauriError,
  info as tauriInfo,
  trace as tauriTrace,
  warn as tauriWarn,
} from '@tauri-apps/plugin-log';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { getLoggerLevel, logger, setLoggerLevel } from './logger';

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
      {
        level: 'trace',
        run: () => logger.trace('Test trace'),
        plugin: tauriTrace,
        expected: ['Test trace'],
      },
      {
        level: 'debug',
        run: () => logger.debug('Test debug', { data: 123 }),
        plugin: tauriDebug,
        expected: ['"component":"Client"', '"data":123'],
      },
      {
        level: 'info',
        run: () => logger.info('Test info'),
        plugin: tauriInfo,
        expected: ['Test info'],
      },
      {
        level: 'warn',
        run: () => logger.warn('Test warn', 'warning', 42),
        plugin: tauriWarn,
        expected: ['"args":["warning",42]'],
      },
      {
        level: 'error',
        run: () => logger.error('Test error', new Error('test')),
        plugin: tauriError,
        expected: ['Test error'],
      },
    ] as const;

    for (const { level, run, plugin, expected } of cases) {
      setLoggerLevel(level);
      await run();
      expect(plugin).toHaveBeenCalledTimes(1);
      for (const fragment of expected) {
        expect(plugin).toHaveBeenCalledWith(expect.stringContaining(fragment));
      }
    }
  });

  it('allows debug messages when configured for debug', async () => {
    setLoggerLevel('debug');

    await logger.debug('Test message', { data: 123 });

    expect(tauriDebug).toHaveBeenCalledWith(expect.stringContaining('"data":123'));
  });

  it('allows trace messages only when configured for trace', async () => {
    await logger.trace('Trace skipped');
    expect(tauriTrace).not.toHaveBeenCalled();

    setLoggerLevel('trace');
    await logger.trace('Trace written');

    expect(tauriTrace).toHaveBeenCalledWith(expect.stringContaining('Trace written'));
  });

  it('filters lower-priority messages when configured for warn', async () => {
    setLoggerLevel('warn');

    await logger.debug('Debug skipped');
    await logger.info('Info skipped');
    await logger.warn('Warn written');
    await logger.error('Error written');

    expect(tauriDebug).not.toHaveBeenCalled();
    expect(tauriInfo).not.toHaveBeenCalled();
    expect(tauriWarn).toHaveBeenCalledWith(expect.stringContaining('Warn written'));
    expect(tauriError).toHaveBeenCalledWith(expect.stringContaining('Error written'));
  });

  it('serializes structured context safely and redacts sensitive data', async () => {
    setLoggerLevel('debug');
    const circular: Record<string, unknown> = {
      nested: new Error('request https://host/?api_key=private'),
    };
    circular.self = circular;
    await logger.debug('[BatchQueue] task_start', circular);
    const entry = JSON.parse(String(vi.mocked(tauriDebug).mock.calls[0][0]));
    expect(entry).toMatchObject({
      schema_version: 1,
      source: 'frontend',
      component: 'BatchQueue',
      event: 'task_start',
    });
    expect(entry.context.nested.message).toBe('request [redacted-url]');
    expect(entry.context.self).toBe('[circular]');
  });

  it('does not reject when the log plugin fails', async () => {
    vi.mocked(tauriError).mockRejectedValueOnce(new Error('plugin unavailable'));
    await expect(logger.error('failed')).resolves.toBeUndefined();
  });

  it('redacts context before both sinks and preserves error causes and bigint', async () => {
    const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    try {
      await logger.error('[EmbySync] task_failed', {
        taskId: 'task-1',
        count: 3n,
        error: new Error('decode failed', { cause: new Error('timeout') }),
        credentials: {
          apiKey: 'private-key',
          authorization: 'Bearer private-token',
          webhookSecret: 'private-secret',
        },
        config: { private: 'private-config' },
        transcript: 'private-transcript',
        requestHeaders: { 'x-api-key': 'private-header', userToken: 'private-user-token' },
      });
      const written = String(vi.mocked(tauriError).mock.calls[0][0]);
      const entry = JSON.parse(written);
      expect(entry.context).toMatchObject({
        taskId: 'task-1',
        count: '3',
        error: { message: 'decode failed', cause: { message: 'timeout' } },
      });
      expect(written).not.toContain('private-');
      expect(consoleSpy).toHaveBeenCalledWith(written);
    } finally {
      consoleSpy.mockRestore();
    }
  });

  it('nulls absent context values and omits absent Error causes', async () => {
    await logger.info('[BatchQueue] task_start', {
      itemId: undefined,
      error: new Error('boom'),
    });

    const entry = JSON.parse(String(vi.mocked(tauriInfo).mock.calls[0][0]));
    expect(entry.context.itemId).toBeNull();
    expect(entry.context.error).toMatchObject({ name: 'Error', message: 'boom' });
    expect('cause' in entry.context.error).toBe(false);
  });

  it('redacts quoted assignments and non-http stream URLs', async () => {
    await logger.info(
      'probe api_key="private-json" rtsp://admin:private-stream@camera.local:554/live'
    );

    const written = String(vi.mocked(tauriInfo).mock.calls[0][0]);
    expect(written).not.toContain('private');
    expect(written).toContain('[redacted-url]');
  });

  it('keeps writing after a hostile getter or console failure', async () => {
    const consoleSpy = vi.spyOn(console, 'warn').mockImplementation(() => {
      throw new Error('console unavailable');
    });
    try {
      const value = Object.defineProperty({}, 'field', {
        enumerable: true,
        get() {
          throw new Error('unreadable');
        },
      });
      await expect(logger.warn('context failed', value)).resolves.toBeUndefined();
      expect(tauriWarn).toHaveBeenCalledWith(expect.stringContaining('[unserializable]'));
    } finally {
      consoleSpy.mockRestore();
    }
  });
});
