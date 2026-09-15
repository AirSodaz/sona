import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { ModelInfo } from '../../../types/modelCatalog';
import { formatModelModeTag, ModelCard, resolveUniqueModeTags } from '../ModelCard';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string } & Record<string, unknown>) => {
      if (typeof options?.defaultValue === 'string') {
        return options.defaultValue;
      }
      return key;
    },
  }),
}));

const mockModel: ModelInfo = {
  id: 'sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17',
  name: 'SenseVoice',
  description: 'settings.descriptions.sensevoice',
  type: 'sensevoice',
  engine: 'sherpa-onnx',
  size: '228 MB',
  languages: ['zh', 'en', 'ja', 'ko', 'yue'],
  languageMode: 'selectable',
  groupId: 'sensevoice',
  versionLabel: 'Int8',
  rules: {
    requiresVad: true,
    requiresPunctuation: false,
  },
  artifacts: [],
};

describe('ModelCard with ModelBrandLogo', () => {
  it('renders the model card with brand logo badge in header', () => {
    const { container } = render(
      <ModelCard
        models={[mockModel]}
        installedModels={new Set([mockModel.id])}
        downloads={{}}
        onDelete={vi.fn()}
        onDownload={vi.fn()}
        onCancelDownload={vi.fn()}
      />
    );

    // Verify identity wrapper and badge exists
    const identityWrapper = container.querySelector('.model-card-identity');
    expect(identityWrapper).toBeTruthy();

    const logoBadge = container.querySelector('.model-card-logo-badge');
    expect(logoBadge).toBeTruthy();

    // Verify brand logo image rendered inside badge
    const img = logoBadge?.querySelector('img');
    expect(img).toBeTruthy();
    expect(img?.getAttribute('width')).toBe('36');
    expect(img?.getAttribute('alt')).toBe('SenseVoice');

    // Verify model title
    expect(screen.getByText(/SenseVoice \(Int8\)/)).toBeTruthy();
  });

  it('renders family brand logo when multiple versions are present', () => {
    const fp32Model: ModelInfo = {
      ...mockModel,
      id: 'sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17',
      versionLabel: 'Fp32',
    };

    const { container } = render(
      <ModelCard
        models={[mockModel, fp32Model]}
        installedModels={new Set([mockModel.id])}
        downloads={{}}
        onDelete={vi.fn()}
        onDownload={vi.fn()}
        onCancelDownload={vi.fn()}
      />
    );

    const logoBadge = container.querySelector('.model-card-logo-badge');
    expect(logoBadge).toBeTruthy();
    expect(logoBadge?.querySelector('img')).toBeTruthy();

    // In multi-version family card, the header shows the base family name
    expect(screen.getByText('SenseVoice')).toBeTruthy();
  });

  it('does not render logo badge for non-ASR models (e.g. VAD or punctuation)', () => {
    const vadModel: ModelInfo = {
      ...mockModel,
      id: 'silero-vad',
      name: 'Silero - VAD',
      type: 'vad',
      groupId: 'vad',
      versionLabel: undefined,
    };

    const { container } = render(
      <ModelCard
        models={[vadModel]}
        installedModels={new Set([vadModel.id])}
        downloads={{}}
        onDelete={vi.fn()}
        onDownload={vi.fn()}
        onCancelDownload={vi.fn()}
      />
    );

    expect(container.querySelector('.model-card-logo-badge')).toBeNull();
    expect(screen.getByText('Silero - VAD')).toBeTruthy();
  });
});

describe('ModelCard mode tags', () => {
  it('renders "Live" tag instead of "Streaming" for models with streaming mode', () => {
    const streamingModel: ModelInfo = {
      ...mockModel,
      modes: ['streaming'],
    };

    render(
      <ModelCard
        models={[streamingModel]}
        installedModels={new Set([streamingModel.id])}
        downloads={{}}
        onDelete={vi.fn()}
        onDownload={vi.fn()}
        onCancelDownload={vi.fn()}
      />
    );

    expect(screen.getByText('Live')).toBeTruthy();
    expect(screen.queryByText('Streaming')).toBeNull();
  });

  it('renders "Live" and "Batch" tags without duplication', () => {
    const dualModeModel: ModelInfo = {
      ...mockModel,
      modes: ['streaming', 'batch'],
    };

    render(
      <ModelCard
        models={[dualModeModel]}
        installedModels={new Set([dualModeModel.id])}
        downloads={{}}
        onDelete={vi.fn()}
        onDownload={vi.fn()}
        onCancelDownload={vi.fn()}
      />
    );

    expect(screen.getByText('Live')).toBeTruthy();
    expect(screen.getByText('Batch')).toBeTruthy();
    expect(screen.queryByText('Streaming')).toBeNull();
  });

  it('formats model mode tags correctly and deduplicates equivalent modes', () => {
    expect(formatModelModeTag('streaming')).toBe('Live');
    expect(formatModelModeTag('live')).toBe('Live');
    expect(formatModelModeTag('batch')).toBe('Batch');
    expect(resolveUniqueModeTags(['streaming', 'live', 'batch'])).toEqual(['Live', 'Batch']);
  });
});
