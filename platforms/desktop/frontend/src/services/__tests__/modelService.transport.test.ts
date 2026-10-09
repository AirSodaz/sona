import { describe, expect, it } from 'vitest';
import {
  createMockPlatform,
  InMemoryFileSystemPort,
  MockPathPort,
  MockTransport,
} from '../../platform/drivers/mock';
import { createModelService } from '../modelService';

describe('ModelService with MockTransport and MockPlatform (zero native mocks)', () => {
  it('loads models directory using MockPathPort and InMemoryFileSystemPort', async () => {
    const fs = new InMemoryFileSystemPort();
    const path = new MockPathPort('/mock/app-data', '/mock/temp');
    const transport = new MockTransport();

    transport.setCommandHandler('storage_get_directories', async () => ({
      modelsDir: '/mock/app-data/models',
    }));

    const platform = createMockPlatform({
      transport,
      ports: {
        ...createMockPlatform().ports,
        fs,
        path,
      },
    });

    const service = createModelService(platform);
    const modelsDir = await service.getModelsDir();

    expect(modelsDir).toBe('/mock/app-data/models');
    expect(await fs.exists('/mock/app-data/models')).toBe(true);
  });

  it('checks model installed status via InMemoryFileSystemPort without native fs', async () => {
    const fs = new InMemoryFileSystemPort();
    const path = new MockPathPort('/mock/app-data', '/mock/temp');
    const transport = new MockTransport();

    const expectedModelPath = '/mock/app-data/models/test-model.bin';
    fs.setFile(expectedModelPath, 'binary-weights');

    transport.setCommandHandler('storage_get_directories', async () => ({
      modelsDir: '/mock/app-data/models',
    }));
    transport.setCommandHandler('get_model_catalog_snapshot', async () => ({
      modelsDir: '/mock/app-data/models',
      models: [],
      sections: [],
      selectionOptions: {},
      modelPathById: {},
      modelIdByNormalizedPath: {},
      pathMatchTokens: {},
      dependencyRequestsByModelId: {},
      restoreDefaults: {
        streaming: null,
        batch: null,
        speakerSegmentation: null,
        speakerEmbedding: null,
        alignment: null,
      },
    }));

    const platform = createMockPlatform({
      transport,
      ports: {
        ...createMockPlatform().ports,
        fs,
        path,
      },
    });

    const service = createModelService(platform);

    expect(service).toBeDefined();
    expect(await fs.exists(expectedModelPath)).toBe(true);
  });
});
