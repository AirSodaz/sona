import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { SettingsGeneralTab } from '../settings/SettingsGeneralTab';
import packageJson from '../../../package.json';

const testContext = vi.hoisted(() => ({
  alertMock: vi.fn().mockResolvedValue(undefined),
  applyImportBackupMock: vi.fn().mockResolvedValue(undefined),
  batchQueueState: {
    queueItems: [] as Array<{ status: string }>,
  },
  confirmMock: vi.fn().mockResolvedValue(false),
  disposePreparedImportMock: vi.fn().mockResolvedValue(undefined),
  exportBackupMock: vi.fn().mockResolvedValue(null),
  listBackupsMock: vi.fn().mockResolvedValue([]),
  loadWebDavConfigMock: vi.fn().mockResolvedValue({
    serverUrl: '',
    remoteDir: '',
    username: '',
    password: '',
  }),
  prepareImportFromRemoteMock: vi.fn().mockResolvedValue(null),
  prepareImportBackupMock: vi.fn().mockResolvedValue(null),
  saveWebDavConfigMock: vi.fn().mockResolvedValue(undefined),
  testWebDavConnectionMock: vi.fn().mockResolvedValue({
    status: 'success',
    message: 'ready',
  }),
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
  uploadWebDavBackupMock: vi.fn(),
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

vi.mock('../settings/SettingsLayout', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../settings/SettingsLayout')>();

  return {
    ...actual,
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
  };
});

vi.mock('../../services/backupService', () => ({
  backupService: {
    applyImportBackup: testContext.applyImportBackupMock,
    disposePreparedImport: testContext.disposePreparedImportMock,
    exportBackup: testContext.exportBackupMock,
    getBackupOperationBlocker: vi.fn(),
    prepareImportBackup: testContext.prepareImportBackupMock,
  },
}));

vi.mock('../../services/backupWebDavService', () => ({
  backupWebDavService: {
    listBackups: testContext.listBackupsMock,
    loadConfig: testContext.loadWebDavConfigMock,
    prepareImportFromRemote: testContext.prepareImportFromRemoteMock,
    saveConfig: testContext.saveWebDavConfigMock,
    testConnection: testContext.testWebDavConnectionMock,
    uploadBackup: testContext.uploadWebDavBackupMock,
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
  const openWebDavAccordion = async () => {
    fireEvent.click(screen.getByRole('button', { name: /WebDAV Cloud Sync/i }));

    await waitFor(() => {
      expect(screen.getByLabelText('Server URL')).toBeDefined();
    });
  };

  beforeEach(() => {
    vi.clearAllMocks();
    testContext.transcriptState.isRecording = false;
    testContext.batchQueueState.queueItems = [];
    testContext.loadWebDavConfigMock.mockResolvedValue({
      serverUrl: '',
      remoteDir: '',
      username: '',
      password: '',
    });
    testContext.listBackupsMock.mockResolvedValue([]);
    testContext.prepareImportFromRemoteMock.mockResolvedValue(null);
    testContext.prepareImportBackupMock.mockResolvedValue(null);
    testContext.confirmMock.mockResolvedValue(false);
  });

  it('keeps the WebDAV controls collapsed until the accordion is expanded', async () => {
    render(<SettingsGeneralTab />);

    await waitFor(() => {
      expect(testContext.loadWebDavConfigMock).toHaveBeenCalledTimes(1);
    });

    const accordionToggle = screen.getByRole('button', { name: /WebDAV Cloud Sync/i });

    expect(accordionToggle.getAttribute('aria-expanded')).toBe('false');
    expect(screen.queryByLabelText('Server URL')).toBeNull();
    expect(screen.queryByRole('button', { name: 'Upload Backup' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Refresh Cloud Backups' })).toBeNull();

    fireEvent.click(accordionToggle);

    await waitFor(() => {
      expect(screen.getByLabelText('Server URL')).toBeDefined();
    });
    expect(accordionToggle.getAttribute('aria-expanded')).toBe('true');
    expect(screen.getByRole('button', { name: 'Upload Backup' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'Refresh Cloud Backups' })).toBeDefined();

    fireEvent.click(accordionToggle);

    await waitFor(() => {
      expect(screen.queryByLabelText('Server URL')).toBeNull();
    });
    expect(accordionToggle.getAttribute('aria-expanded')).toBe('false');
  });

  it('disables local backup and WebDAV transfer actions while live recording is active', async () => {
    testContext.transcriptState.isRecording = true;

    render(<SettingsGeneralTab />);

    await waitFor(() => {
      expect(testContext.loadWebDavConfigMock).toHaveBeenCalledTimes(1);
    });

    await openWebDavAccordion();

    expect((screen.getByRole('button', { name: 'Export Backup' }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole('button', { name: 'Import Backup' }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole('button', { name: 'Upload Backup' }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole('button', { name: 'Refresh Cloud Backups' }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByText('Stop Live Record before exporting or importing backups.')).toBeDefined();
  });

  it('shows an HTTP transport warning for WebDAV endpoints that are not encrypted', async () => {
    testContext.loadWebDavConfigMock.mockResolvedValue({
      serverUrl: 'http://nas.local/dav',
      remoteDir: 'backups',
      username: 'demo',
      password: 'secret',
    });

    render(<SettingsGeneralTab />);

    await openWebDavAccordion();

    await waitFor(() => {
      expect(screen.getByText('This WebDAV endpoint uses HTTP, so credentials and backup archives are not protected in transit.')).toBeDefined();
    });
  });

  it('shows destructive import summary copy and disposes the prepared archive when the user cancels', async () => {
    const prepared = {
      importId: 'import-local',
      archivePath: 'C:\\backups\\sona-backup.tar.bz2',
      manifest: {
        schemaVersion: 1,
        createdAt: '2026-04-29T00:00:00.000Z',
        appVersion: packageJson.version,
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

  it('restores a remote snapshot through the existing destructive confirm flow and disposes it when cancelled', async () => {
    const remoteEntry = {
      href: 'https://dav.example.com/backups/sona-backup-2026-04-29_00-00-00.tar.bz2',
      fileName: 'sona-backup-2026-04-29_00-00-00.tar.bz2',
      size: 2048,
      modifiedAt: '2026-04-29T00:00:00.000Z',
    };
    const prepared = {
      importId: 'import-remote',
      archivePath: 'C:\\backups\\sona-backup.tar.bz2',
      manifest: {
        schemaVersion: 1,
        createdAt: '2026-04-29T00:00:00.000Z',
        appVersion: packageJson.version,
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
      automationRules: [],
      automationProcessedEntries: [],
      analyticsContent: '{}',
    };
    testContext.listBackupsMock.mockResolvedValue([remoteEntry]);
    testContext.prepareImportFromRemoteMock.mockResolvedValue(prepared);
    testContext.confirmMock.mockResolvedValue(false);

    render(<SettingsGeneralTab />);

    await openWebDavAccordion();

    expect((screen.getByRole('button', { name: 'Refresh Cloud Backups' }) as HTMLButtonElement).disabled).toBe(false);

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Refresh Cloud Backups' }));
    });

    await waitFor(() => {
      expect(screen.getByText('sona-backup-2026-04-29_00-00-00.tar.bz2')).toBeDefined();
    });

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Restore' }));
    });

    await waitFor(() => {
      expect(testContext.prepareImportFromRemoteMock).toHaveBeenCalledWith(remoteEntry, {
        serverUrl: '',
        remoteDir: '',
        username: '',
        password: '',
      });
    });
    expect(testContext.confirmMock.mock.calls[0]?.[1]?.details).toContain('restored items may reopen without playback');
    expect(testContext.disposePreparedImportMock).toHaveBeenCalledWith(prepared);
    expect(testContext.applyImportBackupMock).not.toHaveBeenCalled();
  });
});
