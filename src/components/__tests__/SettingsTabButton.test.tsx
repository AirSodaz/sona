import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { SettingsTabButton } from '../settings/SettingsTabButton';

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
        expect(button.className).not.toContain('active');
        expect(screen.getByTestId('icon')).toBeDefined();
        expect(screen.getByText('General Settings')).toBeDefined();
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
        const querySelectorSpy = vi.spyOn(document, 'querySelector').mockReturnValue(mockElement as any);

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
