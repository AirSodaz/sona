import { fireEvent, render, screen } from '@testing-library/react';
import type React from 'react';
import { describe, expect, it, vi } from 'vitest';
import type { ModelCatalogModel } from '../../../types/modelCatalog';
import { LocalModelManagementSection } from '../LocalModelManagementSection';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string }) => options?.defaultValue ?? key,
  }),
}));

vi.mock('../MirrorDownloadPicker', () => ({
  MirrorDownloadPicker: () => <div data-testid="mirror" />,
}));

vi.mock('../ModelCard', () => ({
  ModelCard: ({ models }: { models: ModelCatalogModel[] }) => (
    <div data-testid={`model-card-${models[0]?.id ?? 'empty'}`}>
      {models.map((model) => model.name).join(',')}
    </div>
  ),
}));

vi.mock('../Icons', () => ({
  RestoreIcon: () => <span>icon</span>,
}));

vi.mock('../../Dropdown', () => ({
  Dropdown: ({
    id,
    value,
    onChange,
    options,
  }: {
    id?: string;
    value: string;
    onChange: (value: string) => void;
    options: Array<{ value: string; label: string }>;
  }) => (
    <select
      id={id}
      aria-label={id}
      value={value}
      onChange={(event) => onChange(event.target.value)}
    >
      {options.map((option) => (
        <option key={option.value} value={option.value}>
          {option.label}
        </option>
      ))}
    </select>
  ),
}));

vi.mock('../SettingsLayout', () => ({
  SettingsSection: ({ children, title }: { children: React.ReactNode; title: string }) => (
    <section>
      <h2>{title}</h2>
      {children}
    </section>
  ),
  SettingsAccordion: ({
    title,
    children,
    isOpen,
    defaultOpen,
  }: {
    title: string;
    children: React.ReactNode;
    isOpen?: boolean;
    defaultOpen?: boolean;
  }) => {
    const open = isOpen ?? defaultOpen ?? false;
    return (
      <div data-testid={`accordion-${title}`}>
        <div>{title}</div>
        {open ? <div data-testid={`accordion-open-${title}`}>{children}</div> : null}
      </div>
    );
  },
}));

function makeModel(
  partial: Partial<ModelCatalogModel> & Pick<ModelCatalogModel, 'id' | 'name' | 'type'>
): ModelCatalogModel {
  return {
    engine: 'sherpa-onnx',
    isInstalled: false,
    isRecommended: partial.type === 'asr',
    size: '10 MB',
    ...partial,
  } as ModelCatalogModel;
}

describe('LocalModelManagementSection catalog search', () => {
  const asr = makeModel({ id: 'sensevoice', name: 'SenseVoice', type: 'asr', isRecommended: true });
  const silero = makeModel({
    id: 'silero-vad',
    name: 'Silero VAD',
    type: 'vad',
    isRecommended: true,
  });

  const getSectionGroups = (type: string) => {
    if (type === 'asr') {
      return [{ key: 'asr', models: [asr] }];
    }
    if (type === 'vad') {
      return [{ key: 'vad', models: [silero] }];
    }
    return [];
  };

  it('expands VAD for vad/silero search and hides empty Recognition', () => {
    render(
      <LocalModelManagementSection
        catalogLoadState="ready"
        catalogLoadError={null}
        sectionProps={{
          installedModels: new Set(),
          downloads: {},
          handleDelete: vi.fn(),
          handleDownload: vi.fn(),
          handleCancelDownload: vi.fn(),
          handleDiscardDownload: vi.fn(),
        }}
        localModelActionsDisabled={false}
        getSectionGroups={getSectionGroups as never}
        getSectionStatus={() => ({ type: 'off', text: 'Not installed' })}
        t={(key, options) => (options?.defaultValue as string) ?? key}
      />
    );

    expect(document.getElementById('settings-catalog-vad')).toBeTruthy();
    expect(document.getElementById('settings-catalog-asr')).toBeTruthy();

    fireEvent.change(screen.getByLabelText('Search models…'), {
      target: { value: 'vad' },
    });

    expect(screen.queryByTestId('accordion-open-settings.recognition_models')).toBeNull();
    expect(screen.getByTestId('accordion-open-settings.vad_models')).toBeTruthy();
    expect(screen.getByText('Silero VAD')).toBeTruthy();
    expect(screen.queryByText('No matching models')).toBeNull();
  });

  it('defaults the tag filter to Recommended', () => {
    render(
      <LocalModelManagementSection
        catalogLoadState="ready"
        catalogLoadError={null}
        sectionProps={{
          installedModels: new Set(),
          downloads: {},
          handleDelete: vi.fn(),
          handleDownload: vi.fn(),
          handleCancelDownload: vi.fn(),
          handleDiscardDownload: vi.fn(),
        }}
        localModelActionsDisabled={false}
        getSectionGroups={getSectionGroups as never}
        getSectionStatus={() => ({ type: 'off', text: 'Not installed' })}
        t={(key, options) => (options?.defaultValue as string) ?? key}
      />
    );

    const tagFilter = screen.getByLabelText('settings-model-label-filter') as HTMLSelectElement;
    expect(tagFilter.value).toBe('recommended');
  });
});
