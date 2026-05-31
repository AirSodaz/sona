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

    describe('Keyboard Navigation (options.length <= 10)', () => {
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

    describe('Search and Keyboard Loops (options.length > 10)', () => {
        const largeOptions = Array.from({ length: 12 }, (_, i) => ({
            value: `opt${i + 1}`,
            label: `Language Option ${i + 1}`
        }));

        it('shows search input when options > 10', () => {
            render(
                <Dropdown
                    value=""
                    onChange={mockOnChange}
                    options={largeOptions}
                />
            );
            const trigger = screen.getByRole('button', { expanded: false });
            fireEvent.click(trigger);

            const searchInput = screen.getByPlaceholderText('Search...');
            expect(searchInput).toBeDefined();
        });

        it('filters options dynamically by label and value', async () => {
            render(
                <Dropdown
                    value=""
                    onChange={mockOnChange}
                    options={largeOptions}
                />
            );
            const trigger = screen.getByRole('button', { expanded: false });
            fireEvent.click(trigger);

            const searchInput = screen.getByPlaceholderText('Search...');
            fireEvent.change(searchInput, { target: { value: 'Option 10' } });

            // Only option 10 should be visible
            const visibleOptions = screen.getAllByRole('option');
            expect(visibleOptions).toHaveLength(1);
            expect(visibleOptions[0].textContent).toContain('Language Option 10');
        });

        it('resets search query when dropdown closes and opens again', async () => {
            render(
                <Dropdown
                    value=""
                    onChange={mockOnChange}
                    options={largeOptions}
                />
            );
            const trigger = screen.getByRole('button', { expanded: false });
            fireEvent.click(trigger);

            const searchInput = screen.getByPlaceholderText('Search...');
            fireEvent.change(searchInput, { target: { value: 'Option 10' } });

            // Close by pressing Escape
            fireEvent.keyDown(searchInput, { key: 'Escape', bubbles: true });

            await waitFor(() => {
                expect(screen.queryByRole('listbox')).toBeNull();
            });

            // Reopen
            fireEvent.click(trigger);

            const newSearchInput = await screen.findByPlaceholderText('Search...') as HTMLInputElement;
            expect(newSearchInput.value).toBe('');
        });

        it('navigates keyboard focus loop correctly', async () => {
            render(
                <Dropdown
                    value=""
                    onChange={mockOnChange}
                    options={largeOptions}
                />
            );
            const trigger = screen.getByRole('button', { expanded: false });
            fireEvent.click(trigger);

            const searchInput = screen.getByPlaceholderText('Search...');

            // Wait for initial focus on search input (since showSearch is true)
            await waitFor(() => {
                expect(document.activeElement).toBe(searchInput);
            });

            const visibleOptions = screen.getAllByRole('option');

            // Press ArrowDown on search input -> focuses first option item
            fireEvent.keyDown(searchInput, { key: 'ArrowDown', bubbles: true });
            expect(document.activeElement).toBe(visibleOptions[0]);

            // Press ArrowUp on first option item -> focuses search input
            fireEvent.keyDown(visibleOptions[0], { key: 'ArrowUp', bubbles: true });
            expect(document.activeElement).toBe(searchInput);

            // Navigate to last option item
            // Focus the last option button directly
            visibleOptions[visibleOptions.length - 1].focus();
            expect(document.activeElement).toBe(visibleOptions[visibleOptions.length - 1]);

            // Press ArrowDown on last option item -> loops focus back to search input
            fireEvent.keyDown(visibleOptions[visibleOptions.length - 1], { key: 'ArrowDown', bubbles: true });
            expect(document.activeElement).toBe(searchInput);
        });

        it('redirects character typing back to search input and appends', async () => {
            render(
                <Dropdown
                    value=""
                    onChange={mockOnChange}
                    options={largeOptions}
                />
            );
            const trigger = screen.getByRole('button', { expanded: false });
            fireEvent.click(trigger);

            const searchInput = screen.getByPlaceholderText('Search...') as HTMLInputElement;
            const visibleOptions = screen.getAllByRole('option');

            // Focus first item
            visibleOptions[0].focus();
            expect(document.activeElement).toBe(visibleOptions[0]);

            // Type character 'x'
            fireEvent.keyDown(visibleOptions[0], { key: 'x', bubbles: true });

            expect(document.activeElement).toBe(searchInput);
            expect(searchInput.value).toBe('x');
        });
    });
});
