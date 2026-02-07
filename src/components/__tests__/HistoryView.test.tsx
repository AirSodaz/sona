import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { HistoryView } from '../HistoryView';
import { useHistoryStore } from '../../stores/historyStore';
import { useDialogStore } from '../../stores/dialogStore';

// Mock dependencies
vi.mock('react-i18next', () => ({
    useTranslation: () => ({
        t: (key: string) => key,
    }),
}));

vi.mock('react-virtuoso', () => ({
    Virtuoso: ({ data, itemContent, context }: any) => {
        return (
            <div>
                {data.map((item: any, index: number) => (
                    <div key={item.id || index}>
                        {itemContent(index, item, context)}
                    </div>
                ))}
            </div>
        );
    }
}));

vi.mock('../../stores/historyStore', async () => {
    const { create } = await import('zustand');
    const actual = await vi.importActual('../../stores/historyStore');

    const useHistoryStore = create(() => ({
        items: [],
        isLoading: false,
        element: null,
        loadItems: vi.fn(),
        deleteItem: vi.fn(),
    }));

    return {
        ...actual,
        useHistoryStore
    };
});

vi.mock('../../stores/transcriptStore', () => ({
    useTranscriptStore: (selector: any) => selector({
        setSegments: vi.fn(),
        setAudioUrl: vi.fn(),
    }),
}));

vi.mock('../../services/historyService', () => ({
    historyService: {
        loadTranscript: vi.fn(),
        getAudioUrl: vi.fn(),
    }
}));


describe('HistoryView', () => {
    beforeEach(() => {
        vi.clearAllMocks();
        // Reset dialog store
        useDialogStore.setState({ isOpen: false, options: null, resolveRef: null });

        // Reset history store mock
        useHistoryStore.setState({
            items: [
                {
                    id: 'item-1',
                    title: 'Test Recording 1',
                    timestamp: Date.now(),
                    duration: 60,
                    audioPath: '/path/to/audio1.wav',
                    transcriptPath: '/path/to/transcript1.json',
                    previewText: 'This is a preview of the transcript...'
                }
            ],
            isLoading: false
        });
    });

    it('renders history items', () => {
        render(<HistoryView />);
        expect(screen.getByText('Test Recording 1')).toBeDefined();
    });

    it('requests confirmation before deleting', async () => {
        // Mock confirm to return true
        const confirmSpy = vi.spyOn(useDialogStore.getState(), 'confirm').mockResolvedValue(true);
        const deleteItemSpy = vi.fn();
        useHistoryStore.setState({ deleteItem: deleteItemSpy });

        render(<HistoryView />);

        const deleteBtn = screen.getByRole('button', { name: 'common.delete_item' });
        fireEvent.click(deleteBtn);

        expect(confirmSpy).toHaveBeenCalledWith(
            'history.delete_confirm',
            expect.objectContaining({
                title: 'history.delete_title', // or defaultValue
                variant: 'error'
            })
        );

        // Wait for deleteItem to be called
        await waitFor(() => {
            expect(deleteItemSpy).toHaveBeenCalledWith('item-1');
        });
    });

    it('does not delete if confirmation is cancelled', async () => {
        // Mock confirm to return false
        const confirmSpy = vi.spyOn(useDialogStore.getState(), 'confirm').mockResolvedValue(false);
        const deleteItemSpy = vi.fn();
        useHistoryStore.setState({ deleteItem: deleteItemSpy });

        render(<HistoryView />);

        const deleteBtn = screen.getByRole('button', { name: 'common.delete_item' });
        fireEvent.click(deleteBtn);

        expect(confirmSpy).toHaveBeenCalled();

        // Ensure deleteItem was NOT called
        await new Promise(resolve => setTimeout(resolve, 100)); // wait a bit to be sure
        expect(deleteItemSpy).not.toHaveBeenCalled();
    });
});
