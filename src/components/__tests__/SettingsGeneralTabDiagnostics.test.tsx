import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { SettingsGeneralTab } from '../settings/SettingsGeneralTab';
import { DEFAULT_CONFIG, useConfigStore } from '../../stores/configStore';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) =>
      (options?.defaultValue as string | undefined) ?? key,
  }),
  initReactI18next: {
    type: '3rdParty',
    init: () => undefined,
  },
}));

vi.mock('../Icons', () => ({
  GeneralIcon: () => <div />,
}));

vi.mock('../../services/backupService', () => ({
  backupService: {
    applyImportBackup: vi.fn(),
    disposePreparedImport: vi.fn(),
    exportBackup: vi.fn(),
    prepareImportBackup: vi.fn(),
  },
}));

vi.mock('../Dropdown', () => ({
  Dropdown: ({ id, value, onChange, options, style }: any) => (
    <select id={id} value={value} onChange={(event) => onChange?.(event.target.value)} style={style}>
      {options?.map((option: any) => (
        <option key={option.value} value={option.value}>
          {option.label}
        </option>
      ))}
    </select>
  ),
}));

vi.mock('../Switch', () => ({
  Switch: ({ checked, onChange }: any) => (
    <button type="button" onClick={() => onChange?.(!checked)}>
      {checked ? 'on' : 'off'}
    </button>
  ),
}));

vi.mock('../../stores/batchQueueStore', () => ({
  useBatchQueueStore: (selector: any) => selector({
    queueItems: [],
  }),
}));

vi.mock('../settings/SettingsLayout', () => ({
  SettingsTabContainer: ({ children }: any) => <div>{children}</div>,
  SettingsSection: ({ children, title, description }: any) => (
    <section>
      {title ? <div>{title}</div> : null}
      {description ? <div>{description}</div> : null}
      {children}
    </section>
  ),
  SettingsItem: ({ children, title, hint }: any) => (
    <div>
      {title ? <div>{title}</div> : null}
      {hint ? <div>{hint}</div> : null}
      {children}
    </div>
  ),
  SettingsPageHeader: ({ title, description }: any) => (
    <header>
      <div>{title}</div>
      <div>{description}</div>
    </header>
  ),
}));

vi.mock('../../stores/dialogStore', () => ({
  useDialogStore: (selector: any) => selector({
    alert: vi.fn().mockResolvedValue(undefined),
    confirm: vi.fn().mockResolvedValue(false),
  }),
}));

vi.mock('../../stores/transcriptStore', () => ({
  useTranscriptStore: (selector: any) => selector({
    isRecording: false,
  }),
}));

describe('SettingsGeneralTab diagnostics entry', () => {
  beforeEach(() => {
    useConfigStore.setState({
      config: {
        ...DEFAULT_CONFIG,
      },
    });
  });

  it('calls the external diagnostics opener when the button is clicked', () => {
    const onOpenDiagnostics = vi.fn();

    render(<SettingsGeneralTab onOpenDiagnostics={onOpenDiagnostics} />);

    fireEvent.click(screen.getByRole('button', { name: 'Open Diagnostics' }));

    expect(onOpenDiagnostics).toHaveBeenCalledTimes(1);
  });
});
