import { describe, expect, it } from 'vitest';
import {
  formatChannelBadgeLabel,
  getAppBuildInfo,
  getAppReleaseChannel,
  isUpdateSupported,
} from '../channel';

describe('channel utility', () => {
  it('identifies channels from explicit env variable', () => {
    expect(getAppReleaseChannel('0.8.2', 'nightly')).toBe('nightly');
    expect(getAppReleaseChannel('0.8.2', 'NIGHTLY')).toBe('nightly');
    expect(getAppReleaseChannel('0.8.2', 'ci')).toBe('ci');
    expect(getAppReleaseChannel('0.8.2', 'dev')).toBe('dev');
    expect(getAppReleaseChannel('0.8.2-45', 'stable')).toBe('stable');
  });

  it('infers nightly channel from hyphenated version string when env is not set', () => {
    expect(getAppReleaseChannel('0.8.2-45')).toBe('nightly');
    expect(getAppReleaseChannel('1.0.0-nightly.1')).toBe('nightly');
  });

  it('infers stable channel from standard semver string when env is not set', () => {
    expect(getAppReleaseChannel('0.8.2')).toBe('stable');
    expect(getAppReleaseChannel('1.0.0')).toBe('stable');
  });

  it('formats channel badge labels correctly for all 4 channels', () => {
    expect(formatChannelBadgeLabel('stable')).toBe('Stable');
    expect(formatChannelBadgeLabel('nightly')).toBe('Nightly');
    expect(formatChannelBadgeLabel('ci')).toBe('CI Build');
    expect(formatChannelBadgeLabel('dev')).toBe('Dev');
  });

  it('correctly reports update support per channel', () => {
    expect(isUpdateSupported('stable')).toBe(true);
    expect(isUpdateSupported('nightly')).toBe(true);
    expect(isUpdateSupported('ci')).toBe(false);
    expect(isUpdateSupported('dev')).toBe(false);
  });

  it('returns valid AppBuildInfo fallback when __APP_BUILD_INFO__ is not defined', () => {
    const info = getAppBuildInfo();
    expect(info.semver).toBeDefined();
    expect(info.channel).toBeDefined();
    expect(info.displayVersion).toBeDefined();
    expect(typeof info.isUpdateSupported).toBe('boolean');
  });
});
