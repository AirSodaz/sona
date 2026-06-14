import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { SettingsGeneralTab } from '../settings/SettingsGeneralTab';
import packageJson from '../../../package.json';
import type { PreparedBackupImport } from '../../types/backup';
import { APP_LANGUAGE_OPTIONS } from '../../constants/appLanguages';

const BACKUP_SECTION_LOAD_TIMEOUT_MS = 6000;

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
  showErrorMock: vi.fn().mockResolvedValue(undefined),
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
    showError: testContext.showErrorMock,
  }),
}));

vi.mock('../../stores/transcriptRuntimeStore', () => ({
  useTranscriptRuntimeStore: (selector: any) => selector(testContext.transcriptState),
}));

function buildPreparedImport(importId: string): PreparedBackupImport {
  return {
    importId,
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
    config: {} as PreparedBackupImport['config'],
    projects: [],
    automationRules: [],
    automationProcessedEntries: [],
    analyticsContent: '{}',
  };
}

describe('SettingsGeneralTab backup entry', () => {
  const openWebDavAccordion = async () => {
    const accordionToggle = await screen.findByRole(
      'button',
      { name: /WebDAV Cloud Sync/i },
      { timeout: BACKUP_SECTION_LOAD_TIMEOUT_MS },
    );
    fireEvent.click(accordionToggle);

    await waitFor(() => {
      expect(testContext.loadWebDavConfigMock).toHaveBeenCalledTimes(1);
    });
    await waitFor(() => {
      expect(screen.getByLabelText('Server URL')).toBeDefined();
    });

    return accordionToggle;
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

    expect(screen.getByText('settings.general_title')).toBeDefined();
    expect(testContext.loadWebDavConfigMock).not.toHaveBeenCalled();

    const accordionToggle = await screen.findByRole(
      'button',
      { name: /WebDAV Cloud Sync/i },
      { timeout: BACKUP_SECTION_LOAD_TIMEOUT_MS },
    );
    expect(await screen.findByRole('button', { name: 'Export Backup' })).toBeDefined();
    expect(await screen.findByRole('button', { name: 'Import Backup' })).toBeDefined();
    expect(testContext.loadWebDavConfigMock).not.toHaveBeenCalled();

    expect(accordionToggle.getAttribute('aria-expanded')).toBe('false');
    expect(screen.queryByLabelText('Server URL')).toBeNull();
    expect(screen.queryByRole('button', { name: 'Upload Backup' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Refresh Cloud Backups' })).toBeNull();
    expect(testContext.loadWebDavConfigMock).not.toHaveBeenCalled();

    fireEvent.click(accordionToggle);

    await waitFor(() => {
      expect(testContext.loadWebDavConfigMock).toHaveBeenCalledTimes(1);
    });
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
    expect(testContext.loadWebDavConfigMock).toHaveBeenCalledTimes(1);
  });

  it('shows every supported interface language and saves new language choices', () => {
    render(<SettingsGeneralTab />);

    const languageSelect = document.querySelector('#settings-language') as HTMLSelectElement | null;
    expect(languageSelect).not.toBeNull();
    expect(Array.from(languageSelect?.options ?? []).map((option) => option.value)).toEqual(
      APP_LANGUAGE_OPTIONS.map((option) => option.value),
    );
    expect(Array.from(languageSelect?.options ?? []).map((option) => option.textContent)).toEqual(
      APP_LANGUAGE_OPTIONS.map((option) => option.defaultLabel),
    );

    fireEvent.change(languageSelect as HTMLSelectElement, { target: { value: 'zh-TW' } });
    expect(testContext.updateConfigMock).toHaveBeenLastCalledWith({ appLanguage: 'zh-TW' });

    fireEvent.change(languageSelect as HTMLSelectElement, { target: { value: 'ja' } });
    expect(testContext.updateConfigMock).toHaveBeenLastCalledWith({ appLanguage: 'ja' });
  });

  it('disables local backup and WebDAV transfer actions while live recording is active', async () => {
    testContext.transcriptState.isRecording = true;

    render(<SettingsGeneralTab />);

    await openWebDavAccordion();

    expect((screen.getByRole('button', { name: 'Export Backup' }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole('button', { name: 'Import Backup' }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole('button', { name: 'Upload Backup' }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole('button', { name: 'Refresh Cloud Backups' }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByText('Stop Live Record before exporting or importing backups.')).toBeDefined();
  });

  it('shows an HTTPS-required warning for WebDAV endpoints that are not encrypted', async () => {
    testContext.loadWebDavConfigMock.mockResolvedValue({
      serverUrl: 'http://nas.local/dav',
      remoteDir: 'backups',
      username: 'demo',
      password: 'secret',
    });

    render(<SettingsGeneralTab />);

    await openWebDavAccordion();

    await waitFor(() => {
      expect(screen.getByText('WebDAV cloud sync requires HTTPS to protect credentials and backup archives in transit.')).toBeDefined();
    });
  });

  it('uses the standardized error dialog when backup export fails', async () => {
    const failure = {
      code: 'E_BACKUP_EXPORT',
      error: {
        message: 'Disk is full.',
      },
    };
    testContext.exportBackupMock.mockRejectedValueOnce(failure);

    render(<SettingsGeneralTab />);
    const exportButton = await screen.findByRole('button', { name: 'Export Backup' });

    await act(async () => {
      fireEvent.click(exportButton);
    });

    await waitFor(() => {
      expect(testContext.showErrorMock).toHaveBeenCalledWith({
        code: 'backup.export_failed',
        messageKey: 'errors.backup.export_failed',
        cause: failure,
        titleKey: 'settings.backup.error_title',
      });
    });
    expect(testContext.alertMock).not.toHaveBeenCalled();
  });

  it('shows destructive import summary copy and disposes the prepared archive when the user cancels', async () => {
    const prepared = buildPreparedImport('import-local');
    testContext.prepareImportBackupMock.mockResolvedValue(prepared);
    testContext.confirmMock.mockResolvedValue(false);

    render(<SettingsGeneralTab />);
    const importButton = await screen.findByRole('button', { name: 'Import Backup' });

    await act(async () => {
      fireEvent.click(importButton);
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

  it('applies a local backup and shows the success alert when confirmed', async () => {
    const prepared = buildPreparedImport('import-local-success');
    testContext.prepareImportBackupMock.mockResolvedValue(prepared);
    testContext.confirmMock.mockResolvedValue(true);

    render(<SettingsGeneralTab />);
    const importButton = await screen.findByRole('button', { name: 'Import Backup' });

    await act(async () => {
      fireEvent.click(importButton);
    });

    await waitFor(() => {
      expect(testContext.applyImportBackupMock).toHaveBeenCalledWith(prepared);
    });
    expect(testContext.alertMock).toHaveBeenCalledWith(
      'Backup archive imported successfully.',
      expect.objectContaining({
        variant: 'success',
      }),
    );
    expect(testContext.disposePreparedImportMock).not.toHaveBeenCalled();
    expect(testContext.showErrorMock).not.toHaveBeenCalled();
  });

  it('restores a remote snapshot through the existing destructive confirm flow and disposes it when cancelled', async () => {
    const remoteEntry = {
      href: 'https://dav.example.com/backups/sona-backup-2026-04-29_00-00-00.tar.bz2',
      fileName: 'sona-backup-2026-04-29_00-00-00.tar.bz2',
      size: 2048,
      modifiedAt: '2026-04-29T00:00:00.000Z',
    };
    const prepared = buildPreparedImport('import-remote');
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

  it('disposes a prepared remote snapshot and shows the standardized restore error when apply fails', async () => {
    const remoteEntry = {
      href: 'https://dav.example.com/backups/sona-backup-2026-04-29_00-00-00.tar.bz2',
      fileName: 'sona-backup-2026-04-29_00-00-00.tar.bz2',
      size: 2048,
      modifiedAt: '2026-04-29T00:00:00.000Z',
    };
    const prepared = buildPreparedImport('import-remote-failure');
    const failure = new Error('apply failed');
    testContext.listBackupsMock.mockResolvedValue([remoteEntry]);
    testContext.prepareImportFromRemoteMock.mockResolvedValue(prepared);
    testContext.confirmMock.mockResolvedValue(true);
    testContext.applyImportBackupMock.mockRejectedValueOnce(failure);

    render(<SettingsGeneralTab />);

    await openWebDavAccordion();

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Refresh Cloud Backups' }));
    });

    await waitFor(() => {
      expect(screen.getByText(remoteEntry.fileName)).toBeDefined();
    });

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Restore' }));
    });

    await waitFor(() => {
      expect(testContext.disposePreparedImportMock).toHaveBeenCalledWith(prepared);
    });
    expect(testContext.showErrorMock).toHaveBeenCalledWith({
      code: 'backup.webdav_restore_failed',
      messageKey: 'errors.backup.webdav_restore_failed',
      cause: failure,
      titleKey: 'settings.backup.error_title',
    });
    expect(testContext.alertMock).not.toHaveBeenCalledWith(
      'Backup archive imported successfully.',
      expect.anything(),
    );
  });
});
