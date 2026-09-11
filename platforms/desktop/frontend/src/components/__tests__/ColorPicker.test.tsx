import { fireEvent, render, screen } from '@testing-library/react';
import React from 'react';
import { describe, expect, it, vi } from 'vitest';
import { ColorPicker } from '../ColorPicker';

describe('ColorPicker', () => {
    it('does not render when isOpen is false', () => {
        const anchorRef = { current: document.createElement('button') };
        const { container } = render(
            <ColorPicker
                isOpen={false}
                color="#64748B"
                onChange={vi.fn()}
                onClose={vi.fn()}
                anchorRef={anchorRef}
            />
        );
        expect(container.querySelector('.sona-color-picker-popover')).toBeNull();
    });

    it('renders with rounded corners, theme borders, and RGB/HEX inputs matching Slate #64748B', () => {
        const onChange = vi.fn();
        const onClose = vi.fn();
        const anchor = document.createElement('button');
        document.body.appendChild(anchor);
        const anchorRef = { current: anchor };

        render(
            <ColorPicker
                isOpen={true}
                color="#64748B"
                onChange={onChange}
                onClose={onClose}
                anchorRef={anchorRef}
            />
        );

        // Popover shell has rounded corners and theme border
        const popover = document.querySelector('.sona-color-picker-popover') as HTMLElement;
        expect(popover).toBeDefined();
        expect(popover.style.borderRadius).toBe('12px');
        expect(popover.style.border).toBe('1px solid var(--color-border)');

        // Board has rounded corners and theme border
        const board = document.querySelector('.sona-color-picker-board') as HTMLElement;
        expect(board).toBeDefined();
        expect(board.style.borderRadius).toBe('8px');
        expect(board.style.border).toBe('1px solid var(--color-border)');

        // Slider has rounded corners and theme border
        const hueSlider = document.querySelector('.sona-color-picker-hue') as HTMLElement;
        expect(hueSlider).toBeDefined();
        expect(hueSlider.style.borderRadius).toBe('7px');
        expect(hueSlider.style.border).toBe('1px solid var(--color-border)');

        // RGB inputs match #64748B: R:100, G:116, B:139
        const rInput = screen.getByDisplayValue('100') as HTMLInputElement;
        const gInput = screen.getByDisplayValue('116') as HTMLInputElement;
        const bInput = screen.getByDisplayValue('139') as HTMLInputElement;
        expect(rInput).toBeDefined();
        expect(gInput).toBeDefined();
        expect(bInput).toBeDefined();
        expect(rInput.style.borderRadius).toBe('6px');
        expect(rInput.style.border).toBe('1px solid var(--color-border)');

        // Changing R input updates color
        fireEvent.change(rInput, { target: { value: '200' } });
        expect(onChange).toHaveBeenCalled();

        // Toggling format switches to HEX input
        const toggleBtn = screen.getByLabelText('Toggle RGB / HEX');
        fireEvent.click(toggleBtn);
        const hexInput = document.querySelector('input[type="text"]') as HTMLInputElement;
        expect(hexInput).toBeDefined();
        expect(hexInput.style.borderRadius).toBe('6px');
        expect(hexInput.style.border).toBe('1px solid var(--color-border)');

        // Clean up anchor
        document.body.removeChild(anchor);
    });
    it('hue slider updates color on pointer interaction', () => {
        const onChange = vi.fn();
        const anchor = document.createElement('button');
        document.body.appendChild(anchor);
        const anchorRef = { current: anchor };

        render(
            <ColorPicker
                isOpen={true}
                color="#64748B"
                onChange={onChange}
                onClose={vi.fn()}
                anchorRef={anchorRef}
            />
        );

        const hueSlider = document.querySelector('.sona-color-picker-hue') as HTMLElement;
        expect(hueSlider).toBeDefined();
        expect(hueSlider.style.position).toBe('relative');
        expect(hueSlider.style.flex).toContain('1');

        // Mock getBoundingClientRect
        vi.spyOn(hueSlider, 'getBoundingClientRect').mockReturnValue({
            left: 50,
            top: 100,
            width: 200,
            height: 14,
            right: 250,
            bottom: 114,
            x: 50,
            y: 100,
            toJSON: () => {},
        });

        // Click on hue slider at clientX = 150 (halfway -> 180 deg hue)
        fireEvent.pointerDown(hueSlider, { clientX: 150, pointerId: 1 });
        expect(onChange).toHaveBeenCalled();

        document.body.removeChild(anchor);
    });

    it('eyedropper samples color without prematurely closing the picker on outside clicks', async () => {
        const onChange = vi.fn();
        const onClose = vi.fn();
        const anchor = document.createElement('button');
        document.body.appendChild(anchor);
        const anchorRef = { current: anchor };

        // Mock window.EyeDropper
        const mockOpen = vi.fn().mockResolvedValue({ sRGBHex: '#ff0055' });
        (window as unknown as { EyeDropper: unknown }).EyeDropper = class {
            open = mockOpen;
        };

        render(
            <ColorPicker
                isOpen={true}
                color="#64748B"
                onChange={onChange}
                onClose={onClose}
                anchorRef={anchorRef}
            />
        );

        const pipetteBtn = screen.getByLabelText('Pick color');
        expect(pipetteBtn).toBeDefined();

        // Click pipette to activate EyeDropper
        fireEvent.click(pipetteBtn);
        expect(mockOpen).toHaveBeenCalled();

        // While EyeDropper is open, outside click should NOT trigger onClose
        fireEvent.mouseDown(document.body);
        expect(onClose).not.toHaveBeenCalled();

        // Wait for promise resolution
        await vi.waitFor(() => {
            expect(onChange).toHaveBeenCalledWith('#FF0055');
        });

        document.body.removeChild(anchor);
        delete (window as unknown as { EyeDropper?: unknown }).EyeDropper;
    });
});
