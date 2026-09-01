import { describe, expect, it } from 'vitest';
import { formatChannelBadgeLabel, getAppReleaseChannel } from '../channel';

describe('channel utility', () => {
  it('identifies nightly channel from explicit env variable', () => {
    expect(getAppReleaseChannel('0.8.2', 'nightly')).toBe('nightly');
    expect(getAppReleaseChannel('0.8.2', 'NIGHTLY')).toBe('nightly');
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

  it('formats channel badge label correctly as Stable and Nightly', () => {
    expect(formatChannelBadgeLabel('stable')).toBe('Stable');
    expect(formatChannelBadgeLabel('nightly')).toBe('Nightly');
  });
});
