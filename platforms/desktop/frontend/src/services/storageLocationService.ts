import {
  getPlatform,
  type IPlatformPorts,
  type ITransport,
  type PlatformContext,
  TauriCommand,
} from '../platform';
import type { StorageDirectoriesInfo } from '../types/storage';
import { runGuardedQuit } from './quitGuard';

export interface StorageLocationServicePorts {
  storageGetDirectories: () => Promise<StorageDirectoriesInfo>;
  storageCheckCanMigrate: () => Promise<void>;
  storageMigrateDataDirectory: (
    targetDir: string,
    copyExisting: boolean
  ) => Promise<StorageDirectoriesInfo>;
  storageResetDataDirectory: () => Promise<StorageDirectoriesInfo>;
  storageSetModelsDirectory: (
    targetDir: string,
    moveExisting: boolean
  ) => Promise<StorageDirectoriesInfo>;
  storageResetModelsDirectory: () => Promise<StorageDirectoriesInfo>;
  storageOpenPath: (path: string) => Promise<void>;
  openDialog: (options?: {
    directory?: boolean;
    multiple?: boolean;
    defaultPath?: string;
  }) => Promise<string[] | string | null>;
  relaunch: () => Promise<void>;
  runGuardedQuit: typeof runGuardedQuit;
}

export class StorageLocationService {
  constructor(private readonly ports: StorageLocationServicePorts) {}

  async getDirectories(): Promise<StorageDirectoriesInfo> {
    return this.ports.storageGetDirectories();
  }

  async checkCanMigrate(): Promise<void> {
    await this.ports.storageCheckCanMigrate();
  }

  async selectDirectory(defaultPath?: string): Promise<string | null> {
    const selected = await this.ports.openDialog({
      directory: true,
      multiple: false,
      defaultPath: defaultPath || undefined,
    });
    if (!selected) {
      return null;
    }
    return Array.isArray(selected) ? (selected[0] ?? null) : selected;
  }

  async migrateDataDirectory(
    targetDir: string,
    copyExisting: boolean
  ): Promise<StorageDirectoriesInfo> {
    return this.ports.storageMigrateDataDirectory(targetDir, copyExisting);
  }

  async resetDataDirectory(): Promise<StorageDirectoriesInfo> {
    return this.ports.storageResetDataDirectory();
  }

  async setModelsDirectory(
    targetDir: string,
    moveExisting: boolean
  ): Promise<StorageDirectoriesInfo> {
    return this.ports.storageSetModelsDirectory(targetDir, moveExisting);
  }

  async resetModelsDirectory(): Promise<StorageDirectoriesInfo> {
    return this.ports.storageResetModelsDirectory();
  }

  async openPath(path: string): Promise<void> {
    await this.ports.storageOpenPath(path);
  }

  async relaunchApp(): Promise<void> {
    await this.ports.runGuardedQuit(async () => {
      await this.ports.relaunch();
    });
  }
}

export function buildStorageLocationPortsFromPlatform(
  platform: PlatformContext | { transport: ITransport; ports: IPlatformPorts },
  guardedQuit: typeof runGuardedQuit = runGuardedQuit
): StorageLocationServicePorts {
  return {
    storageGetDirectories: () =>
      platform.transport.invoke<StorageDirectoriesInfo>(TauriCommand.storage.getDirectories),
    storageCheckCanMigrate: () =>
      platform.transport.invoke<void>(TauriCommand.storage.checkCanMigrate),
    storageMigrateDataDirectory: (targetDir, copyExisting) =>
      platform.transport.invoke<StorageDirectoriesInfo>(TauriCommand.storage.migrateDataDirectory, {
        targetDir,
        copyExisting,
      }),
    storageResetDataDirectory: () =>
      platform.transport.invoke<StorageDirectoriesInfo>(TauriCommand.storage.resetDataDirectory),
    storageSetModelsDirectory: (targetDir, moveExisting) =>
      platform.transport.invoke<StorageDirectoriesInfo>(TauriCommand.storage.setModelsDirectory, {
        targetDir,
        moveExisting,
      }),
    storageResetModelsDirectory: () =>
      platform.transport.invoke<StorageDirectoriesInfo>(TauriCommand.storage.resetModelsDirectory),
    storageOpenPath: (path) =>
      platform.transport.invoke<void>(TauriCommand.storage.openPath, { path }),
    openDialog: (options) => platform.ports.dialog.openFile(options),
    relaunch: () => platform.ports.lifecycle.relaunch(),
    runGuardedQuit: guardedQuit,
  };
}

export type StorageLocationServiceInput =
  | StorageLocationServicePorts
  | PlatformContext
  | { transport: ITransport; ports: IPlatformPorts };

export function createStorageLocationService(
  input?: StorageLocationServiceInput
): StorageLocationService {
  if (!input) {
    return new StorageLocationService(buildStorageLocationPortsFromPlatform(getPlatform()));
  }
  if ('transport' in input && 'ports' in input) {
    return new StorageLocationService(buildStorageLocationPortsFromPlatform(input));
  }
  return new StorageLocationService(input as StorageLocationServicePorts);
}

export const storageLocationService = createStorageLocationService();
