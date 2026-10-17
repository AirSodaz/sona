export interface DesktopShimStatus {
  shimDir: string;
  inCurrentPath: boolean;
  inUserPath: boolean;
  isConfigured: boolean;
  installedShims: string[];
  defaultChannel: string;
  currentChannel: string;
  stableInstalled: boolean;
  nightlyInstalled: boolean;
}
