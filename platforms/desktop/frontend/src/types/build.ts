export type AppReleaseChannel = 'stable' | 'nightly' | 'ci' | 'dev';

export interface AppBuildInfo {
  semver: string;
  channel: AppReleaseChannel;
  commitSha: string;
  isDirty: boolean;
  buildNumber: string | null;
  buildTimestamp: string;
  displayVersion: string;
  isUpdateSupported: boolean;
}

declare global {
  // Injected at build time by Vite via `define: { __APP_BUILD_INFO__: ... }`
  const __APP_BUILD_INFO__: AppBuildInfo | undefined;
}
