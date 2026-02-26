import { render, screen, fireEvent } from '@testing-library/react';
import { EditorToolbar } from '../EditorToolbar';
import { useTranscriptStore } from '../../stores/transcriptStore';
import { vi, describe, beforeEach, it, expect } from 'vitest';

// Mock store
vi.mock('../../stores/transcriptStore', () => ({
    useTranscriptStore: vi.fn(),
}));

// Mock translation
vi.mock('react-i18next', () => ({
    useTranslation: () => ({ t: (_key: string, def: string) => def }),
}));

describe('EditorToolbar', () => {
    let execCommandMock: any;

    beforeEach(() => {
        vi.clearAllMocks();
        // Mock document.execCommand
        execCommandMock = vi.fn();
        document.execCommand = execCommandMock;
    });

    it('should not render when not editing', () => {
        (useTranscriptStore as any).mockImplementation((selector: any) => selector({ editingSegmentId: null }));
        const { container } = render(<EditorToolbar />);
        expect(container.firstChild).toBeNull();
    });

    it('should render when editing', () => {
        (useTranscriptStore as any).mockImplementation((selector: any) => selector({ editingSegmentId: 'seg-1' }));
        render(<EditorToolbar />);

        // getByRole throws if not found
        expect(screen.getByRole('button', { name: 'Undo' })).toBeTruthy();
        expect(screen.getByRole('button', { name: 'Bold' })).toBeTruthy();
    });

    it('should call execCommand on button click', () => {
        (useTranscriptStore as any).mockImplementation((selector: any) => selector({ editingSegmentId: 'seg-1' }));
        render(<EditorToolbar />);

        const boldBtn = screen.getByRole('button', { name: 'Bold' });
        fireEvent.click(boldBtn);
        expect(execCommandMock).toHaveBeenCalledWith('bold', false, undefined);

        const italicBtn = screen.getByRole('button', { name: 'Italic' });
        fireEvent.click(italicBtn);
        expect(execCommandMock).toHaveBeenCalledWith('italic', false, undefined);

        const lineBreakBtn = screen.getByRole('button', { name: 'Line break' });
        fireEvent.click(lineBreakBtn);
        expect(execCommandMock).toHaveBeenCalledWith('insertLineBreak', false, undefined);
    });

    it('should prevent default on mouse down to preserve focus', () => {
        (useTranscriptStore as any).mockImplementation((selector: any) => selector({ editingSegmentId: 'seg-1' }));
        render(<EditorToolbar />);

        const boldButton = screen.getByRole('button', { name: 'Bold' });

        const event = new MouseEvent('mousedown', {
            bubbles: true,
            cancelable: true,
        });
        const preventDefaultSpy = vi.spyOn(event, 'preventDefault');

        fireEvent(boldButton, event);

        expect(preventDefaultSpy).toHaveBeenCalled();
    });
});
