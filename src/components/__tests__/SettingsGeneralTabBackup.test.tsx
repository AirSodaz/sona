import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { SettingsGeneralTab } from '../settings/SettingsGeneralTab';

const testContext = vi.hoisted(() => ({
  alertMock: vi.fn().mockResolvedValue(undefined),
  applyImportBackupMock: vi.fn().mockResolvedValue(undefined),
  batchQueueState: {
    queueItems: [] as Array<{ status: string }>,
  },
  confirmMock: vi.fn().mockResolvedValue(false),
  disposePreparedImportMock: vi.fn().mockResolvedValue(undefined),
  exportBackupMock: vi.fn().mockResolvedValue(null),
  prepareImportBackupMock: vi.fn().mockResolvedValue(null),
  transcriptState: {
    isRecording: false,
  },
  updateConfigMock: vi.fn(),
  uiConfig: {
    appLanguage: 'auto',
    theme: 'auto',
    font: 'system',
    minimizeToTrayOnExit: true,
    autoCheckUpdates: true,
  },
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) => {
      const template = (options?.defaultValue as string | undefined) ?? key;
      return template.replace(/\{\{(\w+)\}\}/g, (_match, token: string) => String(options?.[token] ?? `{{${token}}}`));
    },
  }),
  initReactI18next: {
    type: '3rdParty',
    init: () => undefined,
  },
}));

vi.mock('../Icons', () => ({
  GeneralIcon: () => <div />,
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

vi.mock('../../services/backupService', () => ({
  backupService: {
    applyImportBackup: testContext.applyImportBackupMock,
    disposePreparedImport: testContext.disposePreparedImportMock,
    exportBackup: testContext.exportBackupMock,
    getBackupOperationBlocker: vi.fn(),
    prepareImportBackup: testContext.prepareImportBackupMock,
  },
}));

vi.mock('../../stores/batchQueueStore', () => ({
  useBatchQueueStore: (selector: any) => selector(testContext.batchQueueState),
}));

vi.mock('../../stores/configStore', () => ({
  useSetConfig: () => testContext.updateConfigMock,
  useUIConfig: () => testContext.uiConfig,
}));

vi.mock('../../stores/dialogStore', () => ({
  useDialogStore: (selector: any) => selector({
    alert: testContext.alertMock,
    confirm: testContext.confirmMock,
  }),
}));

vi.mock('../../stores/transcriptStore', () => ({
  useTranscriptStore: (selector: any) => selector(testContext.transcriptState),
}));

describe('SettingsGeneralTab backup entry', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    testContext.transcriptState.isRecording = false;
    testContext.batchQueueState.queueItems = [];
    testContext.prepareImportBackupMock.mockResolvedValue(null);
    testContext.confirmMock.mockResolvedValue(false);
  });

  it('disables backup actions while live recording is active', () => {
    testContext.transcriptState.isRecording = true;

    render(<SettingsGeneralTab />);

    expect((screen.getByRole('button', { name: 'Export Backup' }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole('button', { name: 'Import Backup' }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByText('Stop Live Record before exporting or importing backups.')).toBeDefined();
  });

  it('shows destructive import summary copy and disposes the prepared archive when the user cancels', async () => {
    const prepared = {
      archivePath: 'C:\\backups\\sona-backup.tar.bz2',
      extractionDir: 'C:\\temp\\prepared-backup',
      manifest: {
        schemaVersion: 1,
        createdAt: '2026-04-29T00:00:00.000Z',
        appVersion: '0.6.3',
        historyMode: 'light',
        scopes: {
          config: true,
          workspace: true,
          history: true,
          automation: true,
          analytics: true,
        },
        counts: {
          projects: 2,
          historyItems: 5,
          transcriptFiles: 5,
          summaryFiles: 3,
          automationRules: 1,
          automationProcessedEntries: 7,
          analyticsFiles: 1,
        },
      },
      config: {} as any,
      projects: [],
      historyItems: [],
      transcriptFiles: {},
      summaryFiles: {},
      automationRules: [],
      automationProcessedEntries: [],
      analyticsContent: '{}',
    };
    testContext.prepareImportBackupMock.mockResolvedValue(prepared);
    testContext.confirmMock.mockResolvedValue(false);

    render(<SettingsGeneralTab />);

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Import Backup' }));
    });

    await waitFor(() => {
      expect(testContext.confirmMock).toHaveBeenCalledWith(
        'Import this backup and replace the current local data?',
        expect.objectContaining({
          title: 'Replace current data',
          details: expect.stringContaining('Projects: 2'),
        }),
      );
    });
    expect(testContext.confirmMock.mock.calls[0]?.[1]?.details).toContain('restored items may reopen without playback');
    expect(testContext.disposePreparedImportMock).toHaveBeenCalledWith(prepared);
    expect(testContext.applyImportBackupMock).not.toHaveBeenCalled();
  });
});
