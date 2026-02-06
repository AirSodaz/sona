import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { Dropdown } from '../Dropdown';

describe('Dropdown', () => {
    const mockOnChange = vi.fn();
    const options = [
        { value: 'option1', label: 'Option 1' },
        { value: 'option2', label: 'Option 2' },
        { value: 'option3', label: 'Option 3' }
    ];

    beforeEach(() => {
        vi.clearAllMocks();
    });

    it('renders the dropdown with placeholder or value', () => {
        const { rerender } = render(
            <Dropdown
                value=""
                onChange={mockOnChange}
                options={options}
                placeholder="Select something"
            />
        );
        expect(screen.getByText('Select something')).toBeDefined();

        rerender(
            <Dropdown
                value="option1"
                onChange={mockOnChange}
                options={options}
            />
        );
        expect(screen.getByText('Option 1')).toBeDefined();
    });

    it('opens menu when clicked', () => {
        render(
            <Dropdown
                value=""
                onChange={mockOnChange}
                options={options}
            />
        );
        const trigger = screen.getByRole('button', { expanded: false });
        fireEvent.click(trigger);

        expect(screen.getByRole('listbox')).toBeDefined();
        expect(screen.getAllByRole('option')).toHaveLength(3);
    });

    it('selects an option when clicked', () => {
        render(
            <Dropdown
                value=""
                onChange={mockOnChange}
                options={options}
            />
        );
        fireEvent.click(screen.getByRole('button', { expanded: false }));
        fireEvent.click(screen.getByText('Option 2'));

        expect(mockOnChange).toHaveBeenCalledWith('option2');
    });

    describe('Keyboard Navigation', () => {
        it('focuses first item or selected item when opened via keyboard', async () => {
            render(
                <Dropdown
                    value=""
                    onChange={mockOnChange}
                    options={options}
                />
            );
            const trigger = screen.getByRole('button', { expanded: false });
            fireEvent.click(trigger); // Simulate opening

            const optionsList = screen.getAllByRole('option');
            await waitFor(() => {
                expect(document.activeElement).toBe(optionsList[0]);
            });
        });

        it('navigates with arrow keys', async () => {
            render(
                <Dropdown
                    value=""
                    onChange={mockOnChange}
                    options={options}
                />
            );
            const trigger = screen.getByRole('button', { expanded: false });
            fireEvent.click(trigger);

            const optionsList = screen.getAllByRole('option');

            // Wait for initial focus
            await waitFor(() => {
                expect(document.activeElement).toBe(optionsList[0]);
            });

            // Simulate ArrowDown on the container or current focus
            fireEvent.keyDown(optionsList[0], { key: 'ArrowDown', bubbles: true });

            // This expects focus to move to second item
            expect(document.activeElement).toBe(optionsList[1]);
        });

        it('closes on Escape and returns focus to trigger', async () => {
             render(
                <Dropdown
                    value=""
                    onChange={mockOnChange}
                    options={options}
                />
            );
            const trigger = screen.getByRole('button', { expanded: false });
            trigger.focus();
            fireEvent.click(trigger);

            expect(screen.getByRole('listbox')).toBeDefined();

            // Press Escape
            const optionsList = screen.getAllByRole('option');
            fireEvent.keyDown(optionsList[0] || trigger, { key: 'Escape', bubbles: true });

            expect(screen.queryByRole('listbox')).toBeNull();
            expect(document.activeElement).toBe(trigger);
        });
    });
});
