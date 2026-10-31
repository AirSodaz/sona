import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import type * as ReactI18Next from 'react-i18next';
import { afterEach, describe, expect, it, vi } from 'vitest';
import * as mediaServerService from '../../../../../services/mediaServer/mediaServerService';
import type { MediaServerProfile } from '../../../../../types/mediaServer';
import { MediaServerManageModal } from '../MediaServerManageModal';

vi.mock('react-i18next', async (importOriginal) => {
  const actual = await importOriginal<typeof ReactI18Next>();
  return {
    ...actual,
    useTranslation: () => ({
      t: (_key: string, opts?: { defaultValue?: string }) => opts?.defaultValue ?? _key,
      i18n: { language: 'en' },
    }),
  };
});

describe('MediaServerManageModal', () => {
  const mockProfiles: MediaServerProfile[] = [
    {
      id: 'emby-1',
      name: 'Home Emby',
      serverType: 'emby',
      serverUrl: 'http://192.168.1.50:8096',
      apiKey: 'emby-token-123',
      username: 'admin',
      allowInvalidCerts: false,
      createdAt: 1000,
      updatedAt: 1000,
    },
    {
      id: 'jellyfin-1',
      name: 'Remote Jellyfin',
      serverType: 'jellyfin',
      serverUrl: 'https://jellyfin.example.com',
      apiKey: 'jellyfin-token-456',
      allowInvalidCerts: true,
      createdAt: 2000,
      updatedAt: 2000,
    },
  ];

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it('renders list of servers with badges, avatars, and active indicator', () => {
    render(
      <MediaServerManageModal
        isOpen={true}
        onClose={vi.fn()}
        profiles={mockProfiles}
        activeProfileId="emby-1"
        onProfilesUpdated={vi.fn()}
        onSelectActiveProfile={vi.fn()}
      />
    );

    expect(screen.getByText('Home Emby')).toBeDefined();
    expect(screen.getByText('Remote Jellyfin')).toBeDefined();
    expect(screen.getByText('Active')).toBeDefined();
    expect(screen.getByText('http://192.168.1.50:8096')).toBeDefined();
    expect(screen.getByText('https://jellyfin.example.com')).toBeDefined();
  });

  it('renders empty state when no servers configured', () => {
    render(
      <MediaServerManageModal
        isOpen={true}
        onClose={vi.fn()}
        profiles={[]}
        activeProfileId={null}
        onProfilesUpdated={vi.fn()}
        onSelectActiveProfile={vi.fn()}
      />
    );

    expect(screen.getByText('No media servers configured.')).toBeDefined();
  });

  it('switches to add server form when Add Server is clicked and supports cancel/back', () => {
    render(
      <MediaServerManageModal
        isOpen={true}
        onClose={vi.fn()}
        profiles={mockProfiles}
        activeProfileId="emby-1"
        onProfilesUpdated={vi.fn()}
        onSelectActiveProfile={vi.fn()}
      />
    );

    const addBtn = screen.getByRole('button', { name: 'Add Server' });
    fireEvent.click(addBtn);

    expect(screen.getByText('Add Media Server')).toBeDefined();
    expect(screen.getByRole('button', { name: /Emby.*Emby Media Server/i })).toBeDefined();
    expect(screen.getByRole('button', { name: /Jellyfin.*Jellyfin Media Server/i })).toBeDefined();

    // Can toggle back
    const backBtn = screen.getByRole('button', { name: 'Back' });
    fireEvent.click(backBtn);

    expect(screen.getByText('Home Emby')).toBeDefined();
  });

  it('calls onSelectActiveProfile when Select is clicked for inactive server', () => {
    const onSelect = vi.fn();
    render(
      <MediaServerManageModal
        isOpen={true}
        onClose={vi.fn()}
        profiles={mockProfiles}
        activeProfileId="emby-1"
        onProfilesUpdated={vi.fn()}
        onSelectActiveProfile={onSelect}
      />
    );

    const selectBtn = screen.getByRole('button', { name: 'Select' });
    fireEvent.click(selectBtn);

    expect(onSelect).toHaveBeenCalledWith('jellyfin-1');
  });

  it('runs connection test in list and updates test status', async () => {
    vi.spyOn(mediaServerService, 'testMediaServerConnection').mockResolvedValue({
      id: 'emby-1',
      serverName: 'Home Emby',
      version: '4.8.8.0',
      hasSubtitleManagerPlugin: true,
      serverType: 'emby',
    });

    render(
      <MediaServerManageModal
        isOpen={true}
        onClose={vi.fn()}
        profiles={mockProfiles}
        activeProfileId="emby-1"
        onProfilesUpdated={vi.fn()}
        onSelectActiveProfile={vi.fn()}
      />
    );

    const testBtns = screen.getAllByRole('button', { name: /Test/i });
    fireEvent.click(testBtns[0]);

    expect(await screen.findByText(/Connected v4\.8\.8\.0/i)).toBeDefined();
  });

  it('renders test, edit, and delete action buttons matching model-action-icon and data-tooltip styles', () => {
    render(
      <MediaServerManageModal
        isOpen={true}
        onClose={vi.fn()}
        profiles={mockProfiles}
        activeProfileId="emby-1"
        onProfilesUpdated={vi.fn()}
        onSelectActiveProfile={vi.fn()}
      />
    );

    const testBtn = screen.getAllByRole('button', { name: /Test/i })[0];
    const editBtn = screen.getAllByRole('button', { name: /Edit/i })[0];
    const deleteBtn = screen.getAllByRole('button', { name: /Delete/i })[0];

    expect(testBtn.className).toContain('model-action-icon');
    expect(testBtn.className).toContain('model-action-test');
    expect(testBtn.getAttribute('data-tooltip')).toBe('Test Connection');
    expect(testBtn.getAttribute('data-tooltip-pos')).toBe('top');
    expect(testBtn.getAttribute('title')).toBeNull();

    expect(editBtn.className).toContain('model-action-icon');
    expect(editBtn.className).toContain('model-action-edit');
    expect(editBtn.getAttribute('data-tooltip')).toBe('Edit');
    expect(editBtn.getAttribute('data-tooltip-pos')).toBe('top');
    expect(editBtn.getAttribute('title')).toBeNull();

    expect(deleteBtn.className).toContain('model-action-icon');
    expect(deleteBtn.className).toContain('model-action-delete');
    expect(deleteBtn.getAttribute('data-tooltip')).toBe('Delete');
    expect(deleteBtn.getAttribute('data-tooltip-pos')).toBe('top');
    expect(deleteBtn.getAttribute('title')).toBeNull();
  });
});
