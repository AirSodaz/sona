import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { HistoryView } from '../HistoryView';
import { useHistoryStore } from '../../stores/historyStore';
import { useDialogStore } from '../../stores/dialogStore';

// Mock dependencies
vi.mock('react-virtuoso', () => ({
    Virtuoso: ({ data, itemContent }: any) => {
        return (
            <div>
                {data.map((item: any, index: number) => (
                    <div key={item.id || index}>
                        {itemContent(index, item)}
                    </div>
                ))}
            </div>
        );
    }
}));

vi.mock('react-i18next', () => ({
    useTranslation: () => ({
        t: (key: string) => key,
    }),
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

vi.mock('../../stores/transcriptStore', () => {
    const setSegments = vi.fn();
    const setAudioUrl = vi.fn();
    const setSourceHistoryId = vi.fn();
    const loadTranscript = vi.fn();

    const useTranscriptStore = (selector: any) => selector({
        setSegments,
        setAudioUrl,
        setSourceHistoryId,
        loadTranscript,
    });

    useTranscriptStore.getState = () => ({
        setSegments,
        setAudioUrl,
        setSourceHistoryId,
        loadTranscript,
    });

    return { useTranscriptStore };
});

vi.mock('../../services/historyService', () => ({
    historyService: {
        loadTranscript: vi.fn(),
        getAudioUrl: vi.fn(),
        openHistoryFolder: vi.fn(),
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

    it('loads item when clicked', async () => {
        const { historyService } = await import('../../services/historyService');
        (historyService.loadTranscript as any).mockResolvedValue([]);
        (historyService.getAudioUrl as any).mockResolvedValue('blob:url');

        render(<HistoryView />);

        // The aria-label is constructed as `t('common.load') item.title`
        // With the mock t function returning keys, it should be "common.load Test Recording 1"
        const loadBtn = screen.getByRole('button', { name: /common.load Test Recording 1/i });
        fireEvent.click(loadBtn);

        await waitFor(() => {
            expect(historyService.loadTranscript).toHaveBeenCalledWith('/path/to/transcript1.json');
            expect(historyService.getAudioUrl).toHaveBeenCalledWith('/path/to/audio1.wav');
        });
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

    it('opens history folder when button clicked', async () => {
        const { historyService } = await import('../../services/historyService');
        render(<HistoryView />);

        const openBtn = screen.getByRole('button', { name: /history.open_folder/i });
        fireEvent.click(openBtn);

        expect(historyService.openHistoryFolder).toHaveBeenCalled();
    });
});
