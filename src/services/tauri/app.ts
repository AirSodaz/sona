import type { RuntimeEnvironmentStatus, RuntimePathStatus } from '../../types/runtime';
import { TauriCommand } from './commands';
import { invokeTauri } from './invoke';

export interface DownloadFileRequest {
  url: string;
  outputPath: string;
  id: string;
}

export interface ExtractTarBz2Request {
  archivePath: string;
  targetDir: string;
}

export interface UpdateTrayMenuRequest {
  showText: string;
  settingsText: string;
  updatesText: string;
  quitText: string;
  captionText: string;
  captionChecked: boolean;
}

export async function extractTarBz2(request: ExtractTarBz2Request): Promise<void> {
  await invokeTauri<void>(TauriCommand.app.extractTarBz2, request);
}

export async function downloadFile(request: DownloadFileRequest): Promise<void> {
  await invokeTauri<void>(TauriCommand.app.downloadFile, request);
}

export async function cancelDownload(id: string): Promise<void> {
  await invokeTauri<void>(TauriCommand.app.cancelDownload, { id });
}

export async function openLogFolder(): Promise<void> {
  await invokeTauri<void>(TauriCommand.app.openLogFolder);
}

export async function getRuntimeEnvironmentStatus(): Promise<RuntimeEnvironmentStatus> {
  return invokeTauri<RuntimeEnvironmentStatus>(TauriCommand.app.getRuntimeEnvironmentStatus);
}

export async function getPathStatuses(paths: string[]): Promise<RuntimePathStatus[]> {
  return invokeTauri<RuntimePathStatus[]>(TauriCommand.app.getPathStatuses, { paths });
}

export async function hasActiveDownloads(): Promise<boolean> {
  return invokeTauri<boolean>(TauriCommand.app.hasActiveDownloads);
}

export async function forceExit(): Promise<void> {
  await invokeTauri<void>(TauriCommand.app.forceExit);
}

export async function updateTrayMenu(request: UpdateTrayMenuRequest): Promise<void> {
  await invokeTauri<void>(TauriCommand.app.updateTrayMenu, request);
}

export async function setMinimizeToTray(enabled: boolean): Promise<void> {
  await invokeTauri<void>(TauriCommand.app.setMinimizeToTray, { enabled });
}
