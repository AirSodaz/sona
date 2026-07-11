import { logger } from '../utils/logger';

interface ModelFileServicePorts {
  appLocalDataDir: () => Promise<string>;
  join: (...paths: string[]) => Promise<string>;
  exists: (path: string) => Promise<boolean>;
  mkdir: (path: string, options?: { recursive?: boolean }) => Promise<void>;
  remove: (path: string, options?: { recursive?: boolean }) => Promise<void>;
}

class ModelFileService {
  constructor(private readonly ports: ModelFileServicePorts) {}

  async getModelsDir(): Promise<string> {
    const appDataDir = await this.ports.appLocalDataDir();
    const modelsDir = await this.ports.join(appDataDir, 'models');
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
