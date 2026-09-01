import { logger } from '../utils/logger';
import type { StorageDirectoriesInfo } from '../types/storage';

export interface ModelFileServicePorts {
  appLocalDataDir: () => Promise<string>;
  join: (...paths: string[]) => Promise<string>;
  exists: (path: string) => Promise<boolean>;
  mkdir: (path: string, options?: { recursive?: boolean }) => Promise<void>;
  remove: (path: string, options?: { recursive?: boolean }) => Promise<void>;
  getStorageDirectories?: () => Promise<StorageDirectoriesInfo>;
}

export class ModelFileService {
  constructor(private readonly ports: ModelFileServicePorts) {}

  async getModelsDir(): Promise<string> {
    let modelsDir: string;
    if (this.ports.getStorageDirectories) {
      try {
        const dirs = await this.ports.getStorageDirectories();
        modelsDir = dirs.modelsDir;
      } catch {
        const appDataDir = await this.ports.appLocalDataDir();
        modelsDir = await this.ports.join(appDataDir, 'models');
      }
    } else {
      const appDataDir = await this.ports.appLocalDataDir();
      modelsDir = await this.ports.join(appDataDir, 'models');
    }
    try {
      if (!(await this.ports.exists(modelsDir))) {
        await this.ports.mkdir(modelsDir, { recursive: true });
      }
    } catch (error) {
      logger.debug('[ModelFileService] Could not check or create models directory via frontend fs plugin:', error);
    }
    logger.info('[ModelService] Models directory:', modelsDir);
    return modelsDir;
  }

  async removeIfExists(path: string): Promise<void> {
    try {
      if (await this.ports.exists(path)) {
        await this.ports.remove(path, { recursive: true });
      }
    } catch (error) {
      logger.warn('[ModelFileService] Failed to remove path via frontend fs plugin:', error);
    }
  }
}

export function createModelFileService(ports: ModelFileServicePorts): ModelFileService {
  return new ModelFileService(ports);
}
