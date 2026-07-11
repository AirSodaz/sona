import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render } from '@testing-library/react';
import { SettingsModelsPane } from '../SettingsModelsPane';

const useModelManagerMock = vi.hoisted(() => vi.fn(() => ({
    deletingId: null,
    downloads: {},
    installedModels: new Set<string>(),
    modelCatalog: {
        modelsDir: '',
        models: [],
        sections: [],
        selectionOptions: {
            streaming: [],
            batch: [],
            speakerSegmentation: [],
            speakerEmbedding: [],
        },
        modelPathById: {},
        modelIdByNormalizedPath: {},
        pathMatchTokens: [],
        dependencyRequestsByModelId: {},
        restoreDefaults: {
            punctuationModelPath: '',
            speakerSegmentationModelPath: '',
            speakerEmbeddingModelPath: '',
            enableITN: true,
            batchVadEnabled: true,
            vadBufferSize: 5,
            maxConcurrent: 2,
        },
    },
    selectedModelIds: {
        streaming: null,
        batch: null,
        speakerSegmentation: null,
        speakerEmbedding: null,
    },
    catalogLoadState: 'idle',
    catalogLoadError: null,
    handleDownload: vi.fn(),
    handleCancelDownload: vi.fn(),
    handleLoad: vi.fn(),
    handleDelete: vi.fn(),
    restoreDefaultModelSettings: vi.fn(),
})));

vi.mock('../../../hooks/useModelManager', async (importOriginal) => {
    const actual = await importOriginal<typeof import('../../../hooks/useModelManager')>();
    return {
        ...actual,
        useModelManager: useModelManagerMock,
    };
});

vi.mock('../SettingsModelsTab', () => ({
    SettingsModelsTab: ({ isActive }: { isActive?: boolean }) => (
        <div data-testid="models-tab">{String(isActive)}</div>
    ),
}));

describe('SettingsModelsPane', () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it('runs model side effects only when the pane is both open and active', () => {
        render(<SettingsModelsPane isOpen={true} isActive={false} />);

        expect(useModelManagerMock).toHaveBeenLastCalledWith(false);
    });
});
