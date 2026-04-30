import type { RuntimeEnvironmentStatus, RuntimePathStatus } from '../../types/runtime';
import { TauriCommand } from './commands';
import type { TauriCommandArgs } from './contracts';
import { invokeTauri } from './invoke';

export type DownloadFileRequest = TauriCommandArgs<typeof TauriCommand.app.downloadFile>;

export type ExtractTarBz2Request = TauriCommandArgs<typeof TauriCommand.app.extractTarBz2>;

export type UpdateTrayMenuRequest = TauriCommandArgs<typeof TauriCommand.app.updateTrayMenu>;

export async function extractTarBz2(request: ExtractTarBz2Request): Promise<void> {
  await invokeTauri(TauriCommand.app.extractTarBz2, request);
}

export async function downloadFile(request: DownloadFileRequest): Promise<void> {
  await invokeTauri(TauriCommand.app.downloadFile, request);
}

export async function cancelDownload(id: string): Promise<void> {
  await invokeTauri(TauriCommand.app.cancelDownload, { id });
}

export async function openLogFolder(): Promise<void> {
  await invokeTauri(TauriCommand.app.openLogFolder);
}

export async function getRuntimeEnvironmentStatus(): Promise<RuntimeEnvironmentStatus> {
  return invokeTauri(TauriCommand.app.getRuntimeEnvironmentStatus);
}

export async function getPathStatuses(paths: string[]): Promise<RuntimePathStatus[]> {
  return invokeTauri(TauriCommand.app.getPathStatuses, { paths });
}

export async function hasActiveDownloads(): Promise<boolean> {
  return invokeTauri(TauriCommand.app.hasActiveDownloads);
}

export async function forceExit(): Promise<void> {
  await invokeTauri(TauriCommand.app.forceExit);
}

export async function updateTrayMenu(request: UpdateTrayMenuRequest): Promise<void> {
  await invokeTauri(TauriCommand.app.updateTrayMenu, request);
}

export async function setMinimizeToTray(enabled: boolean): Promise<void> {
  await invokeTauri(TauriCommand.app.setMinimizeToTray, { enabled });
}
