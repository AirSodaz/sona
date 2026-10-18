export type ShimChannel = 'stable' | 'nightly';

export interface DesktopShimStatus {
  shimDir: string;
  inCurrentPath: boolean;
  inUserPath: boolean;
  isConfigured: boolean;
  installedShims: string[];
  defaultChannel: ShimChannel;
  currentChannel: ShimChannel;
  stableInstalled: boolean;
  nightlyInstalled: boolean;
}
