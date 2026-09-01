import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { SettingsLocationCard } from '../SettingsLocationCard';

vi.mock('react-i18next', () => ({
    useTranslation: () => ({
        t: (key: string, options?: { defaultValue?: string }) => options?.defaultValue ?? key,
    }),
}));

describe('SettingsLocationCard', () => {
    it('renders location card with title, hint, path and default badges', () => {
        render(
            <SettingsLocationCard
                testId="test-location-card"
                title="Data Directory"
                hint="Stores databases and audio files."
                path="/path/to/data"
                isCustom={false}
            />
        );

        expect(screen.getByText('Data Directory')).toBeDefined();
        expect(screen.getByText('Stores databases and audio files.')).toBeDefined();
        expect(screen.getByText('/path/to/data')).toBeDefined();
        expect(screen.getByText('Default')).toBeDefined();
    });

    it('renders custom badge and displays restore default button when isCustom is true', () => {
        const onRestoreDefault = vi.fn();
        render(
            <SettingsLocationCard
                title="Models Directory"
                path="/custom/models"
                isCustom={true}
                onRestoreDefault={onRestoreDefault}
            />
        );

        expect(screen.getByText('Custom')).toBeDefined();
        const restoreBtn = screen.getByText('Restore Default');
        fireEvent.click(restoreBtn);
        expect(onRestoreDefault).toHaveBeenCalledTimes(1);
    });

    it('does not render restore default button when isCustom is false', () => {
        const onRestoreDefault = vi.fn();
        render(
            <SettingsLocationCard
                title="Models Directory"
                path="/default/models"
                isCustom={false}
                onRestoreDefault={onRestoreDefault}
            />
        );

        expect(screen.queryByText('Restore Default')).toBeNull();
    });

    it('renders ready status badge when isValid is true', () => {
        render(
            <SettingsLocationCard
                title="FFmpeg Path"
                path="/usr/bin/ffmpeg"
                isValid={true}
            />
        );

        expect(screen.getByText('Ready')).toBeDefined();
    });

    it('renders not found status badge when isValid is false', () => {
        render(
            <SettingsLocationCard
                title="FFmpeg Path"
                path="/usr/bin/missing_ffmpeg"
                isValid={false}
            />
        );

        expect(screen.getByText('Not Found')).toBeDefined();
    });

    it('triggers onChangePath when clicking change/browse button', () => {
        const onChangePath = vi.fn();
        render(
            <SettingsLocationCard
                title="Storage Location"
                path="/some/path"
                changeLabel="Change Directory..."
                onChangePath={onChangePath}
            />
        );

        const changeBtn = screen.getByText('Change Directory...');
        fireEvent.click(changeBtn);
        expect(onChangePath).toHaveBeenCalledTimes(1);
    });

    it('triggers onOpenFolder when clicking open folder button', () => {
        const onOpenFolder = vi.fn();
        render(
            <SettingsLocationCard
                title="Storage Location"
                path="/some/path"
                openFolderLabel="Open Folder"
                onOpenFolder={onOpenFolder}
            />
        );

        const openBtn = screen.getByText('Open Folder');
        fireEvent.click(openBtn);
        expect(onOpenFolder).toHaveBeenCalledTimes(1);
    });

    it('disables open folder button when path is empty or isBusy is true', () => {
        const onOpenFolder = vi.fn();
        const { rerender } = render(
            <SettingsLocationCard
                title="Storage Location"
                path=""
                onOpenFolder={onOpenFolder}
            />
        );

        const openBtn = screen.getByText('Open Folder') as HTMLButtonElement;
        expect(openBtn.disabled).toBe(true);

        rerender(
            <SettingsLocationCard
                title="Storage Location"
                path="/some/path"
                isBusy={true}
                onOpenFolder={onOpenFolder}
            />
        );

        const busyOpenBtn = screen.getByText('Open Folder') as HTMLButtonElement;
        expect(busyOpenBtn.disabled).toBe(true);
    });

    it('renders bottom hint with custom color', () => {
        render(
            <SettingsLocationCard
                title="FFmpeg Path"
                path="/missing"
                isValid={false}
                bottomHint="FFmpeg binary was not found on your system."
                bottomHintColor="rgb(239, 68, 68)"
            />
        );

        const hintEl = screen.getByText('FFmpeg binary was not found on your system.');
        expect(hintEl).toBeDefined();
        expect(hintEl.style.color).toBe('rgb(239, 68, 68)');
    });
});
