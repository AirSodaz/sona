import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createModelFileService } from '../modelFileService';

describe('modelFileService', () => {
  const appLocalDataDir = vi.fn();
  const join = vi.fn((...parts: string[]) => Promise.resolve(parts.join('/')));
  const exists = vi.fn();
  const mkdir = vi.fn();
  const remove = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    appLocalDataDir.mockResolvedValue('/app/data');
    exists.mockResolvedValue(false);
    mkdir.mockResolvedValue(undefined);
    remove.mockResolvedValue(undefined);
  });

  it('resolves and creates the app-local models directory', async () => {
    const service = createModelFileService({ appLocalDataDir, join, exists, mkdir, remove });

    await expect(service.getModelsDir()).resolves.toBe('/app/data/models');

    expect(appLocalDataDir).toHaveBeenCalledTimes(1);
    expect(join).toHaveBeenCalledWith('/app/data', 'models');
    expect(exists).toHaveBeenCalledWith('/app/data/models');
    expect(mkdir).toHaveBeenCalledWith('/app/data/models', { recursive: true });
  });

  it('does not recreate the models directory when it already exists', async () => {
    exists.mockResolvedValueOnce(true);
    const service = createModelFileService({ appLocalDataDir, join, exists, mkdir, remove });

    await service.getModelsDir();

    expect(mkdir).not.toHaveBeenCalled();
  });

  it('removes a model path only when it exists', async () => {
    exists.mockResolvedValueOnce(false).mockResolvedValueOnce(true);
    const service = createModelFileService({ appLocalDataDir, join, exists, mkdir, remove });

    await service.removeIfExists('/models/missing');
    await service.removeIfExists('/models/present');

    expect(remove).toHaveBeenCalledTimes(1);
    expect(remove).toHaveBeenCalledWith('/models/present', { recursive: true });
  });
});
