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
    if (!(await this.ports.exists(modelsDir))) {
      await this.ports.mkdir(modelsDir, { recursive: true });
    }
    logger.info('[ModelService] Models directory:', modelsDir);
    return modelsDir;
  }

  async removeIfExists(path: string): Promise<void> {
    if (await this.ports.exists(path)) {
      await this.ports.remove(path, { recursive: true });
    }
  }
}

export function createModelFileService(ports: ModelFileServicePorts): ModelFileService {
  return new ModelFileService(ports);
}
