import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { SettingsModelsTab } from '../SettingsModelsTab';
import { useConfigStore } from '../../../stores/configStore';
import { ModelManagerContext } from '../../../hooks/useModelManager';
import { setTestConfig } from '../../../test-utils/configTestUtils';

vi.mock('react-i18next', () => ({
    useTranslation: () => ({
        t: (key: string, options?: { defaultValue?: string } & Record<string, unknown>) => {
            if (typeof options?.defaultValue === 'string') {
                return options.defaultValue.replace(/\{\{(\w+)\}\}/g, (_: string, variable: string) => String(options?.[variable] ?? ''));
            }
            return key;
        },
    }),
    initReactI18next: {
        type: '3rdParty',
        init: () => undefined,
    },
}));

vi.mock('../ModelCard', () => ({
    ModelCard: ({ models }: { models: Array<{ id: string; name: string }> }) => (
        <div data-testid={`model-card-${models[0].id}`}>{models[0].name}</div>
    ),
}));

vi.mock('../../../services/modelService', () => ({
    PRESET_MODELS: [
        {
            id: 'sherpa-onnx-pyannote-segmentation-3-0',
            name: 'Pyannote 3.0',
            description: 'settings.descriptions.speaker_segmentation',
            url: 'https://example.com/seg.tar.bz2',
            type: 'speaker-segmentation',
            language: 'multi',
            size: '6.64 MB',
            engine: 'sherpa-onnx',
            rules: { requiresVad: false, requiresPunctuation: false },
        },
        {
            id: '3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx',
            name: '3DSpeaker CAMPPlus',
            description: 'settings.descriptions.speaker_embedding',
            url: 'https://example.com/embed.onnx',
            type: 'speaker-embedding',
            language: 'zh,en',
            size: '27 MB',
            isArchive: false,
            filename: '3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx',
            engine: 'sherpa-onnx',
            rules: { requiresVad: false, requiresPunctuation: false },
        },
    ],
    modelService: {
        getModelPath: vi.fn(async (id: string) => `/models/${id}`),
        getModelRules: vi.fn(() => ({ requiresVad: false, requiresPunctuation: false })),
    },
}));

function renderTab(installedModels: Set<string>) {
    const managerValue = {
        deletingId: null,
        downloads: {},
        installedModels,
        handleDelete: vi.fn(),
        handleDownload: vi.fn(),
        handleCancelDownload: vi.fn(),
        handleLoad: vi.fn(),
        isModelSelected: vi.fn(),
        restoreDefaultModelSettings: vi.fn(),
    } as any;

    return render(
        <ModelManagerContext.Provider value={managerValue}>
            <SettingsModelsTab />
        </ModelManagerContext.Provider>,
    );
}

describe('SettingsModelsTab speaker model selections', () => {
    beforeEach(() => {
        vi.clearAllMocks();
        setTestConfig({
            speakerSegmentationModelPath: '',
            speakerEmbeddingModelPath: '',
        });
    });

    it('shows Off for both speaker dropdowns even when no speaker models are installed', async () => {
        renderTab(new Set());

        await waitFor(() => {
            expect(screen.getByRole('button', { name: 'Speaker Segmentation Model' }).textContent).toContain('Off');
            expect(screen.getByRole('button', { name: 'Speaker Embedding Model' }).textContent).toContain('Off');
        });

        fireEvent.click(screen.getByRole('button', { name: 'Speaker Segmentation Model' }));
        expect(screen.getAllByRole('option').map((option) => option.textContent)).toEqual(['Off']);
    });

    it('clears selected speaker model paths when Off is chosen', async () => {
        setTestConfig({
            speakerSegmentationModelPath: '/models/sherpa-onnx-pyannote-segmentation-3-0',
            speakerEmbeddingModelPath: '/models/3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx',
        });

        renderTab(new Set([
            'sherpa-onnx-pyannote-segmentation-3-0',
            '3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx',
        ]));

        await waitFor(() => {
            expect(screen.getByRole('button', { name: 'Speaker Segmentation Model' }).textContent).toContain('Pyannote 3.0');
            expect(screen.getByRole('button', { name: 'Speaker Embedding Model' }).textContent).toContain('3DSpeaker CAMPPlus');
        });

        fireEvent.click(screen.getByRole('button', { name: 'Speaker Segmentation Model' }));
        fireEvent.click(screen.getByRole('option', { name: 'Off' }));

        fireEvent.click(screen.getByRole('button', { name: 'Speaker Embedding Model' }));
        fireEvent.click(screen.getByRole('option', { name: 'Off' }));

        await waitFor(() => {
            expect(useConfigStore.getState().config.speakerSegmentationModelPath).toBe('');
            expect(useConfigStore.getState().config.speakerEmbeddingModelPath).toBe('');
            expect(screen.getByRole('button', { name: 'Speaker Segmentation Model' }).textContent).toContain('Off');
            expect(screen.getByRole('button', { name: 'Speaker Embedding Model' }).textContent).toContain('Off');
        });
    });
});
