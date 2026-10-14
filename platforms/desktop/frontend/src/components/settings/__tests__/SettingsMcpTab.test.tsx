import { act, fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { TauriCommand } from '../../../services/tauri/commands';
import { invokeTauri } from '../../../services/tauri/invoke';
import { useConfigStore } from '../../../stores/configStore';
import { useMcpStore } from '../../../stores/mcpStore';
import { SettingsMcpTab } from '../SettingsMcpTab';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) => {
      const value = String(options?.defaultValue ?? key);
      return value.replace(/{{(\w+)}}/g, (_match, token: string) => String(options?.[token] ?? ''));
    },
    i18n: { language: 'en' },
  }),
}));

vi.mock('../../../services/tauri/invoke', () => ({
  invokeTauri: vi.fn(),
}));

vi.mock('../../../services/configPersistenceService', () => ({
  persistConfigImmediately: vi.fn(async (patch) => {
    useConfigStore.getState().setConfig(patch);
  }),
}));

const mockInvokeTauri = vi.mocked(invokeTauri);

describe('SettingsMcpTab', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useConfigStore.setState({
      config: {
        ...useConfigStore.getState().config,
        mcpEnabled: true,
        mcpPermissionMode: 'full',
        mcpAllowRecording: true,
        mcpAllowTranscriptEdit: true,
        mcpAllowDestructive: false,
        mcpAllowSettingsUpdate: false,
        mcpAllowModelManagement: true,
        mcpNotifyOnMutation: true,
        mcpAuditLogEnabled: true,
      },
    });

    useMcpStore.setState({
      status: {
        enabled: true,
        running: true,
        endpoint: '\\\\.\\pipe\\sona-agent-ipc',
        activeConnections: 1,
        binaryPath: 'C:\\Program Files\\Sona\\sona-mcp.exe',
        binaryExists: true,
        permissionMode: 'full',
      },
      logs: [
        {
          id: 'log-1',
          timestamp: Date.now(),
          method: 'sona_query_history',
          category: 'read_only',
          status: 'success',
          durationMs: 15,
          paramsSummary: 'limit: 20',
          errorMessage: null,
        },
      ],
      tools: [
        {
          name: 'sona_get_client_state',
          title: 'Get Client State',
          description: 'Query Sona desktop client state',
          category: 'read_only',
          allowed: true,
        },
        {
          name: 'sona_delete_history',
          title: 'Delete History',
          description: 'Delete history record',
          category: 'destructive',
          allowed: true,
        },
      ],
      clientConfigs: {
        binaryPath: 'C:\\Program Files\\Sona\\sona-mcp.exe',
        binaryExists: true,
        endpoint: '\\\\.\\pipe\\sona-agent-ipc',
        guides: [
          {
            id: 'claude-desktop',
            name: 'Claude Desktop',
            configPath: '%APPDATA%\\Claude\\claude_desktop_config.json',
            configContent: '{\n  "mcpServers": {}\n}',
            format: 'json',
          },
          {
            id: 'cursor',
            name: 'Cursor',
            configPath: '%USERPROFILE%\\.cursor\\mcp.json',
            configContent: '{\n  "mcpServers": {}\n}',
            format: 'json',
          },
        ],
      },
      filter: {
        status: 'all',
        query: '',
      },
      fetchStatus: vi.fn().mockResolvedValue(undefined),
      fetchLogs: vi.fn().mockResolvedValue(undefined),
      fetchTools: vi.fn().mockResolvedValue(undefined),
      fetchClientConfigs: vi.fn().mockResolvedValue(undefined),
      isLoading: false,
      error: null,
    });
  });

  it('renders status card and lifecycle buttons', async () => {
    await act(async () => {
      render(<SettingsMcpTab />);
    });

    expect(screen.getByText('MCP Agents')).toBeDefined();
    expect(screen.getByText('Service Lifecycle & Status')).toBeDefined();
    expect(screen.getByText('Listening')).toBeDefined();
    expect(screen.getByText('1 Agent connected')).toBeDefined();
    expect(screen.getByText('\\\\.\\pipe\\sona-agent-ipc')).toBeDefined();
  });

  it('renders permission mode selection and granular options when selected', async () => {
    await act(async () => {
      render(<SettingsMcpTab />);
    });

    const readOnlyBtn = screen.getByText('Read-Only Protection').closest('button');
    expect(readOnlyBtn).toBeDefined();

    const granularBtn = screen.getByText('Custom Granular Permissions').closest('button')!;
    await act(async () => {
      fireEvent.click(granularBtn);
    });

    expect(
      screen.getByText('Allow audio recording control (start/stop/pause/resume)')
    ).toBeDefined();
    expect(
      screen.getByText(
        'Allow editing transcripts and generating summaries (edit_transcript/patch_segments/save_summary)'
      )
    ).toBeDefined();
    expect(
      screen.getByText('Allow destructive actions (delete history/delete project/discard draft)')
    ).toBeDefined();
  });

  it('renders client integration guide tabs and code snippet', async () => {
    await act(async () => {
      render(<SettingsMcpTab />);
    });

    expect(screen.getByText('Client Integration Guide')).toBeDefined();
    expect(screen.getByText('Claude Desktop')).toBeDefined();
    expect(screen.getByText('Cursor')).toBeDefined();
    expect(screen.getByText('%APPDATA%\\Claude\\claude_desktop_config.json')).toBeDefined();
  });

  it('renders activity audit logs and allows clearing logs', async () => {
    mockInvokeTauri.mockResolvedValueOnce(undefined as never);

    await act(async () => {
      render(<SettingsMcpTab />);
    });

    expect(screen.getByText('Live Activity & Audit Stream')).toBeDefined();
    expect(screen.getByText('sona_query_history')).toBeDefined();
    expect(screen.getByText('limit: 20')).toBeDefined();

    const clearBtn = screen.getByText('Clear Logs').closest('button')!;
    await act(async () => {
      fireEvent.click(clearBtn);
    });

    expect(mockInvokeTauri).toHaveBeenCalledWith(TauriCommand.mcp.clearActivityLogs);
  });
});
