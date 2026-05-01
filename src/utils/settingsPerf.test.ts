import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  SETTINGS_PERF_STORAGE_KEY,
  clearSettingsPerf,
  disableSettingsPerf,
  enableSettingsPerf,
  getSettingsPerfErrorDetail,
  isSettingsPerfEnabled,
  markSettingsPerf,
  snapshotSettingsPerf,
} from './settingsPerf';

const loggerInfoMock = vi.hoisted(() => vi.fn().mockResolvedValue(undefined));

vi.mock('./logger', () => ({
  logger: {
    info: loggerInfoMock,
  },
}));

describe('settingsPerf', () => {
  let nowSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    localStorage.clear();
    clearSettingsPerf();
    loggerInfoMock.mockClear();
    nowSpy = vi.spyOn(performance, 'now');
  });

  afterEach(() => {
    nowSpy.mockRestore();
    localStorage.clear();
    clearSettingsPerf();
  });

  it('is disabled by default and does not record marks', () => {
    nowSpy.mockReturnValue(100);

    markSettingsPerf('settings.preload.all.start');

    expect(isSettingsPerfEnabled()).toBe(false);
    expect(snapshotSettingsPerf()).toEqual([]);
    expect(loggerInfoMock).not.toHaveBeenCalled();
  });

  it('records enabled marks with stable timing fields and log format', () => {
    enableSettingsPerf();
    nowSpy.mockReturnValueOnce(100).mockReturnValueOnce(137.456);

    markSettingsPerf('settings.open.default.click', { tab: 'general', source: 'header' });
    markSettingsPerf('settings.shell.commit');

    expect(localStorage.getItem(SETTINGS_PERF_STORAGE_KEY)).toBe('true');
    expect(snapshotSettingsPerf()).toEqual([
      {
        name: 'settings.open.default.click',
        atMs: 100,
        sinceFirstMs: 0,
        sinceOpenMs: 0,
        detail: { tab: 'general', source: 'header' },
      },
      {
        name: 'settings.shell.commit',
        atMs: 137.46,
        sinceFirstMs: 37.46,
        sinceOpenMs: 37.46,
      },
    ]);
    expect(loggerInfoMock).toHaveBeenNthCalledWith(
      1,
      '[SettingsPerf] settings.open.default.click',
      expect.objectContaining({ name: 'settings.open.default.click' }),
    );
    expect(loggerInfoMock).toHaveBeenNthCalledWith(
      2,
      '[SettingsPerf] settings.shell.commit',
      expect.objectContaining({ name: 'settings.shell.commit' }),
    );
  });

  it('stops recording after disable without clearing existing events', () => {
    enableSettingsPerf();
    nowSpy.mockReturnValueOnce(10).mockReturnValueOnce(20);

    markSettingsPerf('settings.open.default.click');
    disableSettingsPerf();
    markSettingsPerf('settings.shell.commit');

    expect(isSettingsPerfEnabled()).toBe(false);
    expect(snapshotSettingsPerf()).toHaveLength(1);
    expect(loggerInfoMock).toHaveBeenCalledTimes(1);
  });

  it('formats error details without throwing on non-Error values', () => {
    expect(getSettingsPerfErrorDetail(new TypeError('boom'))).toEqual({
      name: 'TypeError',
      message: 'boom',
    });
    expect(getSettingsPerfErrorDetail('plain failure')).toEqual({
      message: 'plain failure',
    });
  });
});
