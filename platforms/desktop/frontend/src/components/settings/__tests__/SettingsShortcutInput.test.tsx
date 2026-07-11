import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { SettingsShortcutInput } from '../SettingsShortcutInput';

vi.mock('react-i18next', () => ({
    useTranslation: () => ({
        t: (_key: string, options?: { defaultValue?: string }) => options?.defaultValue ?? _key,
    }),
}));

describe('SettingsShortcutInput', () => {
    it('labels the manual edit button and edit input', () => {
        render(<SettingsShortcutInput value="Ctrl + Space" onChange={vi.fn()} />);

        const editButton = screen.getByRole('button', { name: 'Edit manually' });
        expect(editButton.getAttribute('data-tooltip')).toBe('Edit manually');

        fireEvent.click(editButton);

        expect(screen.getByRole('textbox', { name: 'Edit manually' })).toBeDefined();
    });
});
