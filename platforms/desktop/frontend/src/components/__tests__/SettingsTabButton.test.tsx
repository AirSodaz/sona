import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { SettingsSidebarGroupHeader, SettingsTabButton } from '../settings/SettingsTabButton';

describe('SettingsTabButton', () => {
  const mockSetActiveTab = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders with correct state and id', () => {
    render(
      <SettingsTabButton
        id="general"
        label="General Settings"
        Icon={() => <span data-testid="icon">icon</span>}
        activeTab="dashboard"
        setActiveTab={mockSetActiveTab}
      />
    );

    const button = screen.getByRole('tab');
    expect(button).toBeDefined();
    expect(button.getAttribute('aria-selected')).toBe('false');
    expect(button.getAttribute('title')).toBeNull();
    expect(button.className).not.toContain('active');
    screen.getByTestId('icon');
    const labelSpan = screen.getByText('General Settings');
    expect(labelSpan.className).toBe('settings-tab-btn-text');
  });
  it('calls setActiveTab when clicking an inactive tab', () => {
    render(
      <SettingsTabButton
        id="general"
        label="General Settings"
        Icon={() => <span>icon</span>}
        activeTab="dashboard"
        setActiveTab={mockSetActiveTab}
      />
    );

    const button = screen.getByRole('tab');
    fireEvent.click(button);

    expect(mockSetActiveTab).toHaveBeenCalledWith('general');
  });

  it('scrolls the settings content container to top when clicking an active tab', () => {
    const mockScrollTo = vi.fn();
    const mockElement = { scrollTo: mockScrollTo };
    const querySelectorSpy = vi
      .spyOn(document, 'querySelector')
      .mockReturnValue(mockElement as any);

    render(
      <SettingsTabButton
        id="general"
        label="General Settings"
        Icon={() => <span>icon</span>}
        activeTab="general"
        setActiveTab={mockSetActiveTab}
      />
    );

    const button = screen.getByRole('tab');
    fireEvent.click(button);

    // Should not set active tab again
    expect(mockSetActiveTab).not.toHaveBeenCalled();

    // Should query container and scroll to top
    expect(querySelectorSpy).toHaveBeenCalledWith('.settings-content-scroll');
    expect(mockScrollTo).toHaveBeenCalledWith({ top: 0, behavior: 'smooth' });

    querySelectorSpy.mockRestore();
  });
});

describe('SettingsSidebarGroupHeader', () => {
  it('renders with presentation role and label text', () => {
    const { container } = render(<SettingsSidebarGroupHeader label="General" first={true} />);

    const header = container.querySelector('.settings-sidebar-group-header');
    expect(header).toBeDefined();
    expect(header?.getAttribute('role')).toBe('presentation');
    expect(header?.textContent).toBe('General');
    expect(header?.className).not.toContain('settings-sidebar-group-separator');
  });

  it('includes separator class when not the first group', () => {
    const { container } = render(
      <SettingsSidebarGroupHeader label="Audio & Recognition" first={false} />
    );

    const header = container.querySelector('.settings-sidebar-group-header');
    expect(header).toBeDefined();
    expect(header?.className).toContain('settings-sidebar-group-separator');
    expect(header?.textContent).toBe('Audio & Recognition');
  });
});
