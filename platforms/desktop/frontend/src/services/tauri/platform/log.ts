export type PluginLogModule = typeof import('@tauri-apps/plugin-log');

let pluginLogModulePromise: Promise<PluginLogModule | null> | null = null;

export function getPluginLogModule(): Promise<PluginLogModule | null> {
  if (!pluginLogModulePromise) {
    pluginLogModulePromise = import('@tauri-apps/plugin-log')
      .then((module) => module)
      .catch(() => null);
  }

  return pluginLogModulePromise;
}
