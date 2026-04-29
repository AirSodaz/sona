import { render, screen, fireEvent, act } from '@testing-library/react';
import { EditorToolbar } from '../EditorToolbar';
import { useTranscriptStore } from '../../stores/transcriptStore';
import { vi, describe, beforeEach, afterEach, it, expect } from 'vitest';

vi.mock('../../stores/transcriptStore', () => ({
    useTranscriptStore: vi.fn(),
}));

vi.mock('react-i18next', () => ({
    useTranslation: () => ({
        t: (_key: string, fallbackOrOptions?: string | { defaultValue?: string }) => {
            if (typeof fallbackOrOptions === 'string') {
                return fallbackOrOptions;
            }

            return fallbackOrOptions?.defaultValue || _key;
        },
    }),
}));

describe('EditorToolbar', () => {
    let execCommandMock: any;
    let mockState: any;
    const flushMicrotasks = async () => {
        await act(async () => {
            await Promise.resolve();
        });
    };

    beforeEach(() => {
        vi.clearAllMocks();
        execCommandMock = vi.fn();
        document.execCommand = execCommandMock;

        mockState = {
            editingSegmentId: null,
            sourceHistoryId: null,
            autoSaveStates: {},
        };

        (useTranscriptStore as any).mockImplementation((selector: any) => selector(mockState));
    });

    afterEach(() => {
        vi.useRealTimers();
    });

    it('does not render when there is no saved item and no active edit session', () => {
        const { container } = render(<EditorToolbar />);
        expect(container.firstChild).toBeNull();
    });

    it('does not synthesize a saved status for an opened history item without an auto-save record', () => {
        mockState.sourceHistoryId = 'hist-1';

        const { container } = render(<EditorToolbar />);

        expect(container.firstChild).toBeNull();
        expect(screen.queryByRole('status')).toBeNull();
        expect(screen.queryByRole('button', { name: 'Undo' })).toBeNull();
    });

    it('does not show a save pill for unsaved content while still exposing edit controls', () => {
        mockState.editingSegmentId = 'seg-1';

        render(<EditorToolbar />);

        expect(screen.queryByRole('status')).toBeNull();
        expect(screen.getByRole('button', { name: 'Undo' })).toBeTruthy();
    });

    it('shows a lightweight saving status while auto-save is running', () => {
        mockState.sourceHistoryId = 'hist-1';
        mockState.autoSaveStates = {
            'hist-1': {
                status: 'saving',
                updatedAt: Date.now(),
            },
        };

        render(<EditorToolbar />);

        expect(screen.getByRole('status').textContent).toContain('Saving...');
        expect(screen.queryByRole('button', { name: 'Undo' })).toBeNull();
    });

    it('hides the saved status after 1.5 seconds', async () => {
        vi.useFakeTimers();
        mockState.sourceHistoryId = 'hist-1';
        mockState.autoSaveStates = {
            'hist-1': {
                status: 'saved',
                updatedAt: Date.now(),
            },
        };

        render(<EditorToolbar />);
        await flushMicrotasks();

        expect(screen.getByRole('status').textContent).toContain('Saved');

        await act(async () => {
            await vi.advanceTimersByTimeAsync(1500);
        });

        expect(screen.queryByRole('status')).toBeNull();
    });

    it('renders editor controls while editing and reflects auto-save errors', () => {
        mockState.editingSegmentId = 'seg-1';
        mockState.sourceHistoryId = 'hist-1';
        mockState.autoSaveStates = {
            'hist-1': {
                status: 'error',
                updatedAt: Date.now(),
            },
        };

        render(<EditorToolbar />);

        expect(screen.getByRole('status').textContent).toContain('Save failed');
        expect(screen.getByRole('button', { name: 'Undo' })).toBeTruthy();
        expect(screen.getByRole('button', { name: 'Bold' })).toBeTruthy();
    });

    it('calls execCommand on button click', () => {
        mockState.editingSegmentId = 'seg-1';

        render(<EditorToolbar />);

        fireEvent.click(screen.getByRole('button', { name: 'Bold' }));
        expect(execCommandMock).toHaveBeenCalledWith('bold', false, undefined);

        fireEvent.click(screen.getByRole('button', { name: 'Italic' }));
        expect(execCommandMock).toHaveBeenCalledWith('italic', false, undefined);

        fireEvent.click(screen.getByRole('button', { name: 'Line break' }));
        expect(execCommandMock).toHaveBeenCalledWith('insertLineBreak', false, undefined);
    });

    it('prevents default on mouse down to preserve focus', () => {
        mockState.editingSegmentId = 'seg-1';

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
