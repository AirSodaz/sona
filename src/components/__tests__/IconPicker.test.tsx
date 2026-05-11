import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { IconPicker } from '../IconPicker';

vi.mock('react-i18next', () => ({
    useTranslation: () => ({
        t: (_key: string, options?: { defaultValue?: string }) => options?.defaultValue ?? _key,
    }),
}));

describe('IconPicker', () => {
    it('labels the custom emoji input with the same text as its placeholder', () => {
        render(<IconPicker icon="" onChange={vi.fn()} />);

        fireEvent.click(screen.getByRole('button'));

        const customEmojiInput = screen.getByRole('textbox', { name: 'Custom emoji' });

        expect(customEmojiInput.getAttribute('placeholder')).toBe('Custom emoji');
    });
});
