import { useCallback, RefObject } from 'react';
import { useTranslation } from 'react-i18next';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useDialogStore } from '../stores/dialogStore';
import { TranscriptSegment } from '../types/transcript';

interface UseTranscriptActionsProps {
    /** Stable ref to the current segments list. */
    segmentsRef: RefObject<TranscriptSegment[]>;
    /** Function to request re-alignment of a segment. */
    requestAlignment: (id: string) => void;
}

/**
 * Hook to handle transcript actions like save, delete, and merge.
 * Encapsulates the business logic and dialog interactions for these actions.
 *
 * @param props Hook props.
 * @return Object containing action handlers.
 */
export function useTranscriptActions({ segmentsRef, requestAlignment }: UseTranscriptActionsProps): {
    handleSave: (id: string, text: string) => void;
    handleDelete: (id: string) => Promise<void>;
    handleMergeWithNext: (id: string) => Promise<void>;
} {
    const { t } = useTranslation();
    const { confirm } = useDialogStore();

    const updateSegment = useTranscriptStore((state) => state.updateSegment);
    const deleteSegment = useTranscriptStore((state) => state.deleteSegment);
    const mergeSegments = useTranscriptStore((state) => state.mergeSegments);
    const setEditingSegmentId = useTranscriptStore((state) => state.setEditingSegmentId);

    const handleSave = useCallback((id: string, text: string) => {
        // Check if text actually changed and alignment is possible
        const segment = segmentsRef.current?.find(s => s.id === id);
        const textChanged = segment && segment.text !== text;

        updateSegment(id, { text });
        setEditingSegmentId(null);

        // Trigger re-alignment if text changed and segment has token data
        if (textChanged && segment?.tokens && segment.tokens.length > 0) {
            const config = useTranscriptStore.getState().config;
            if (config.ctcModelPath) {
                requestAlignment(id);
            }
        }
    }, [updateSegment, setEditingSegmentId, requestAlignment, segmentsRef]);

    const handleDelete = useCallback(async (id: string) => {
        const confirmed = await confirm(t('editor.delete_confirm_message', { defaultValue: 'Are you sure you want to delete this segment?' }), {
            title: t('editor.delete_confirm_title', { defaultValue: 'Confirm Delete' }),
            variant: 'warning'
        });

        if (confirmed) {
            deleteSegment(id);
        }
    }, [deleteSegment, t, confirm]);

    const handleMergeWithNext = useCallback(async (id: string) => {
        const confirmed = await confirm(t('editor.merge_confirm_message', { defaultValue: 'Merge this segment with the next one?' }), {
            title: t('editor.merge_confirm_title', { defaultValue: 'Confirm Merge' }),
            variant: 'info'
        });

        if (confirmed) {
            const currentSegments = segmentsRef.current || [];
            const index = currentSegments.findIndex((s) => s.id === id);
            if (index !== -1 && index < currentSegments.length - 1) {
                mergeSegments(id, currentSegments[index + 1].id);
            }
        }
    }, [mergeSegments, t, confirm, segmentsRef]);

    return {
        handleSave,
        handleDelete,
        handleMergeWithNext
    };
}
