import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type * as ReactI18Next from 'react-i18next';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type * as EmbyServiceModule from '../../../../services/emby/embyService';
import * as embyService from '../../../../services/emby/embyService';
import type * as MediaServerServiceModule from '../../../../services/mediaServer/mediaServerService';
import * as mediaServerService from '../../../../services/mediaServer/mediaServerService';
import type {
  MediaServerInfo,
  MediaServerLibrary,
  MediaServerProfile,
} from '../../../../types/mediaServer';
import { ToolMediaServerSyncPanel } from '../ToolMediaServerSyncPanel';

vi.mock('react-i18next', async (importOriginal) => {
  const actual = await importOriginal<typeof ReactI18Next>();
  return {
    ...actual,
    useTranslation: () => ({
      t: (key: string, opts?: { defaultValue?: string }) => opts?.defaultValue ?? key,
      i18n: { language: 'zh-CN' },
    }),
  };
});

vi.mock('../../../../services/mediaServer/mediaServerService', async (importOriginal) => {
  const actual = await importOriginal<typeof MediaServerServiceModule>();
  return {
    ...actual,
    listMediaServerProfiles: vi.fn(),
    setActiveMediaServerProfile: vi.fn(),
    testMediaServerConnection: vi.fn(),
    getMediaServerLibraries: vi.fn(),
    getMediaServerItems: vi.fn(),
    downloadMediaServerAudio: vi.fn(),
    uploadMediaServerSubtitle: vi.fn(),
  };
});

vi.mock('../../../../services/emby/embyService', async (importOriginal) => {
  const actual = await importOriginal<typeof EmbyServiceModule>();
  return {
    ...actual,
    getEmbyConfig: vi.fn(),
    saveEmbyConfig: vi.fn(),
    testEmbyConnection: vi.fn(),
    loginEmby: vi.fn(),
    getEmbyLibraries: vi.fn(),
    getEmbyItems: vi.fn(),
  };
});

const mockEmbyProfile: MediaServerProfile = {
  id: 'profile-emby-1',
  name: 'Home Emby',
  serverType: 'emby',
  serverUrl: 'http://192.168.1.100:8096',
  apiKey: 'token-emby',
  allowInvalidCerts: false,
  createdAt: 1000,
  updatedAt: 1000,
};

const mockJellyfinProfile: MediaServerProfile = {
  id: 'profile-jf-2',
  name: 'Remote Jellyfin',
  serverType: 'jellyfin',
  serverUrl: 'https://jellyfin.example.com',
  apiKey: 'token-jf',
  allowInvalidCerts: true,
  createdAt: 2000,
  updatedAt: 2000,
};

const mockEmbyInfo: MediaServerInfo = {
  id: 'server_emby_123',
  serverName: 'Home Emby Server',
  version: '4.8.8.0',
  hasSubtitleManagerPlugin: false, // plugin missing
  serverType: 'emby',
};

const mockJellyfinInfo: MediaServerInfo = {
  id: 'server_jf_456',
  serverName: 'Remote Jellyfin Server',
  version: '10.9.1',
  hasSubtitleManagerPlugin: true, // native support
  serverType: 'jellyfin',
};

const mockEmbyLibraries: MediaServerLibrary[] = [
  { id: 'lib_emby_1', name: 'Emby Movies', collectionType: 'movies' },
];

const mockJellyfinLibraries: MediaServerLibrary[] = [
  { id: 'lib_jf_1', name: 'Jellyfin Series', collectionType: 'tvshows' },
];

describe('ToolMediaServerSyncPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
  });

  afterEach(() => {
    cleanup();
  });

  it('renders plugin missing banner for Emby when plugin is absent', async () => {
    vi.mocked(mediaServerService.listMediaServerProfiles).mockResolvedValue([mockEmbyProfile]);
    vi.mocked(embyService.testEmbyConnection).mockResolvedValue(mockEmbyInfo);
    vi.mocked(embyService.getEmbyLibraries).mockResolvedValue(mockEmbyLibraries);
    vi.mocked(embyService.getEmbyItems).mockResolvedValue({
      totalRecordCount: 0,
      items: [],
    });

    render(<ToolMediaServerSyncPanel />);

    await waitFor(() => {
      expect(screen.getByText('Home Emby Server (v4.8.8.0)')).toBeDefined();
    });

    // Emby without plugin must display the warning banner
    expect(screen.getByText(/Server plugin Emby.SubtitleManager is not detected/i)).toBeDefined();
    expect(screen.getByText('Plugin Guide')).toBeDefined();
  });

  it('suppresses plugin missing banner and shows native active pill for Jellyfin', async () => {
    vi.mocked(mediaServerService.listMediaServerProfiles).mockResolvedValue([mockJellyfinProfile]);
    vi.mocked(embyService.testEmbyConnection).mockResolvedValue(mockJellyfinInfo);
    vi.mocked(embyService.getEmbyLibraries).mockResolvedValue(mockJellyfinLibraries);
    vi.mocked(embyService.getEmbyItems).mockResolvedValue({
      totalRecordCount: 0,
      items: [],
    });

    render(<ToolMediaServerSyncPanel />);

    await waitFor(() => {
      expect(screen.getByText('Remote Jellyfin Server (v10.9.1)')).toBeDefined();
    });

    // Warning banner should NOT be rendered for Jellyfin
    expect(screen.queryByText(/Server plugin Emby.SubtitleManager is not detected/i)).toBeNull();
    expect(screen.queryByText('Plugin Guide')).toBeNull();

    // Native integration indicator must be present
    expect(screen.getByText(/Native Jellyfin Subtitle Integration Active/i)).toBeDefined();
  });

  it('switching profiles re-triggers connection and library loading for the new server', async () => {
    vi.mocked(mediaServerService.listMediaServerProfiles).mockResolvedValue([
      mockEmbyProfile,
      mockJellyfinProfile,
    ]);
    vi.mocked(mediaServerService.setActiveMediaServerProfile).mockResolvedValue(undefined);

    // Initial load as Emby
    vi.mocked(embyService.testEmbyConnection).mockResolvedValueOnce(mockEmbyInfo);
    vi.mocked(embyService.getEmbyLibraries).mockResolvedValueOnce(mockEmbyLibraries);
    vi.mocked(embyService.getEmbyItems).mockResolvedValueOnce({
      totalRecordCount: 0,
      items: [],
    });

    render(<ToolMediaServerSyncPanel />);

    await waitFor(() => {
      expect(screen.getByText('Home Emby Server (v4.8.8.0)')).toBeDefined();
    });

    // Setup mock responses for Jellyfin profile switch
    vi.mocked(embyService.testEmbyConnection).mockResolvedValueOnce(mockJellyfinInfo);
    vi.mocked(embyService.getEmbyLibraries).mockResolvedValueOnce(mockJellyfinLibraries);
    vi.mocked(embyService.getEmbyItems).mockResolvedValueOnce({
      totalRecordCount: 0,
      items: [],
    });

    // Select dropdown to switch server
    const dropdownTrigger = screen.getByRole('button', { name: /\[Emby\] Home Emby/i });
    fireEvent.click(dropdownTrigger);

    const jellyfinOption = screen.getByRole('option', {
      name: /\[Jellyfin\] Remote Jellyfin/i,
    });
    fireEvent.click(jellyfinOption);

    await waitFor(() => {
      expect(mediaServerService.setActiveMediaServerProfile).toHaveBeenCalledWith('profile-jf-2');
      expect(screen.getByText('Remote Jellyfin Server (v10.9.1)')).toBeDefined();
    });
  });
});
