import type React from 'react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Virtuoso, type VirtuosoHandle } from 'react-virtuoso';
import { useStore } from 'zustand';
import { useTranscriptScrollFollow } from '../../hooks/useTranscriptScrollFollow';
import { useTranscriptUIState } from '../../hooks/useTranscriptUIState';
import { resolveCurrentSessionAudioPath } from '../../services/sessionAudioService';
import { useDialogStore } from '../../stores/dialogStore';
import { useSearchStore } from '../../stores/searchStore';
import { useTranscriptPlaybackStore } from '../../stores/transcriptPlaybackStore';
import { useTranscriptSessionStore } from '../../stores/transcriptSessionStore';
import { areSpeakerTagsEqual } from '../../types/speakerNormalization';
import type { TranscriptSegment } from '../../types/transcript';
import { EditorToolbar } from '../EditorToolbar';
import { PlusCircleIcon } from '../Icons';
import { SearchUI } from '../SearchUI';
import { EnrollSpeakerSampleModal } from './EnrollSpeakerSampleModal';
import { SegmentItem } from './SegmentItem';
import { TranscriptAnticipationIndicator } from './TranscriptAnticipationIndicator';
import { TranscriptScrollFollowBadge } from './TranscriptScrollFollowBadge';
import { TranscriptUIContext } from './TranscriptUIContext';

function TranscriptListHeader(): React.JSX.Element {
  return (
    <div className="transcript-list-header">
      <div className="transcript-list-opening-spacer" aria-hidden="true" />
    </div>
  );
}

function TranscriptListFooter(): React.JSX.Element {
  return (
    <div className="transcript-list-footer">
      <TranscriptAnticipationIndicator />
      <div className="transcript-list-footer-spacer" aria-hidden="true" />
    </div>
  );
}

/** Context passed to virtualized list items via Virtuoso. */
interface TranscriptContext {
  onSeek: (time: number) => void;
  onEdit: (id: string) => void;
  onSave: (id: string, text: string) => void;
  onSaveTranslation: (id: string, translation: string) => void;
  onDelete: (id: string) => void;
  onMergeWithNext: (id: string) => void;
  onSplit: (id: string, leftText: string, rightText: string) => void;
  onAnimationEnd: (id: string) => void;
  onEnrollSample: (segment: TranscriptSegment) => void;
}

interface TranscriptEditorProps {
  /** Whether this editor's shell is currently visible and owns keyboard shortcuts/search UI. */
  isActive?: boolean;
}

/**
 * Editor component for displaying and managing transcript segments.
 *
 * Uses virtualization for performance with large transcripts.
 * Optimized to minimize re-renders during high-frequency updates.
 *
 * @return The transcript editor interface.
 */
export function TranscriptEditor({ isActive = true }: TranscriptEditorProps): React.JSX.Element {
  const { t } = useTranslation();
  const { confirm } = useDialogStore();
  const virtuosoRef = useRef<VirtuosoHandle>(null);

  const segments = useTranscriptSessionStore((state) => state.segments);
  const editingSegmentId = useTranscriptSessionStore((state) => state.editingSegmentId);
  const setEditingSegmentId = useTranscriptSessionStore((state) => state.setEditingSegmentId);
  const updateSegment = useTranscriptSessionStore((state) => state.updateSegment);
  const deleteSegment = useTranscriptSessionStore((state) => state.deleteSegment);
  const mergeSegments = useTranscriptSessionStore((state) => state.mergeSegments);
  const splitTranscriptSegment = useTranscriptSessionStore((state) => state.splitTranscriptSegment);
  const requestSeek = useTranscriptPlaybackStore((state) => state.requestSeek);

  // Hooks for UI state and alignment
  const { uiStore, handleAnimationEnd } = useTranscriptUIState(segments);
  const isDocumentLocked = useStore(
    uiStore,
    useCallback((state) => Boolean(state.isDocumentLocked), [])
  );

  useEffect(() => {
    if (isDocumentLocked && editingSegmentId) {
      setEditingSegmentId(null);
    }
  }, [editingSegmentId, isDocumentLocked, setEditingSegmentId]);

  const [enrollTargetSegment, setEnrollTargetSegment] = useState<TranscriptSegment | null>(null);
  const [resolvedAudioPath, setResolvedAudioPath] = useState<string | null>(null);

  const handleEnrollSample = useCallback(async (segment: TranscriptSegment) => {
    const audioPath = await resolveCurrentSessionAudioPath();
    setResolvedAudioPath(audioPath);
    setEnrollTargetSegment(segment);
  }, []);
  // Keep a ref to segments to make callbacks stable where needed
  const segmentsRef = useRef(segments);

  useEffect(() => {
    segmentsRef.current = segments;
  }, [segments]);

  // Unified scroll follow: streaming output auto-scroll + playback active centering
  const { detachMode, resumeFollow, atBottomStateChange, onUserInteraction } =
    useTranscriptScrollFollow({
      virtuosoRef,
      segments,
      editingSegmentId,
    });

  const handleSeek = useCallback(
    (time: number) => {
      requestSeek(time);
    },
    [requestSeek]
  );

  const handleEdit = useCallback(
    (id: string) => {
      if (isDocumentLocked) return;
      setEditingSegmentId(id);
    },
    [isDocumentLocked, setEditingSegmentId]
  );

  const handleSave = useCallback(
    (id: string, text: string) => {
      updateSegment(id, { text });
      setEditingSegmentId(null);
    },
    [setEditingSegmentId, updateSegment]
  );

  const handleSaveTranslation = useCallback(
    (id: string, translation: string) => {
      if (isDocumentLocked) return;
      updateSegment(id, { translation, draftTranslation: undefined });
    },
    [isDocumentLocked, updateSegment]
  );

  const handleDelete = useCallback(
    async (id: string) => {
      if (isDocumentLocked) return;
      const confirmed = await confirm(
        t('editor.delete_confirm_message', {
          defaultValue: 'Are you sure you want to delete this segment?',
        }),
        {
          title: t('editor.delete_confirm_title', { defaultValue: 'Confirm Delete' }),
          variant: 'warning',
        }
      );

      if (confirmed) {
        deleteSegment(id);
      }
    },
    [t, confirm, deleteSegment, isDocumentLocked]
  );

  const handleMergeWithNext = useCallback(
    async (id: string) => {
      if (isDocumentLocked) return;
      const confirmed = await confirm(
        t('editor.merge_confirm_message', {
          defaultValue: 'Merge this segment with the next one?',
        }),
        {
          title: t('editor.merge_confirm_title', { defaultValue: 'Confirm Merge' }),
          variant: 'info',
        }
      );

      if (confirmed) {
        const currentSegments = segmentsRef.current;
        const index = currentSegments.findIndex((s) => s.id === id);
        if (index !== -1 && index < currentSegments.length - 1) {
          mergeSegments(id, currentSegments[index + 1].id);
        }
      }
    },
    [confirm, isDocumentLocked, mergeSegments, t]
  );

  const handleSplit = useCallback(
    (id: string, leftText: string, rightText: string) => {
      if (isDocumentLocked) return;
      splitTranscriptSegment(id, leftText, rightText);
    },
    [isDocumentLocked, splitTranscriptSegment]
  );

  // Stable context for Virtuoso items (callbacks only)
  const contextValue = useMemo<TranscriptContext>(
    () => ({
      onSeek: handleSeek,
      onEdit: handleEdit,
      onSave: handleSave,
      onSaveTranslation: handleSaveTranslation,
      onDelete: handleDelete,
      onMergeWithNext: handleMergeWithNext,
      onSplit: handleSplit,
      onAnimationEnd: handleAnimationEnd,
      onEnrollSample: handleEnrollSample,
    }),
    [
      handleSeek,
      handleEdit,
      handleSave,
      handleDelete,
      handleMergeWithNext,
      handleSplit,
      handleAnimationEnd,
      handleSaveTranslation,
      handleEnrollSample,
    ]
  );

  const itemContent = useCallback(
    (index: number, segment: TranscriptSegment, context: TranscriptContext) => {
      const previousSegment = index > 0 ? segmentsRef.current[index - 1] : null;
      const nextSegment =
        index < segmentsRef.current.length - 1 ? segmentsRef.current[index + 1] : null;
      return (
        <SegmentItem
          key={segment.id}
          segment={segment}
          index={index}
          showSpeakerLabel={
            Boolean(segment.speaker) &&
            !areSpeakerTagsEqual(previousSegment?.speaker, segment.speaker)
          }
          canMergeWithNext={
            segment.isFinal &&
            (!nextSegment ||
              (nextSegment.isFinal && areSpeakerTagsEqual(segment.speaker, nextSegment.speaker)))
          }
          onSeek={context.onSeek}
          onEdit={context.onEdit}
          onSave={context.onSave}
          onSaveTranslation={context.onSaveTranslation}
          onDelete={context.onDelete}
          onMergeWithNext={context.onMergeWithNext}
          onSplit={context.onSplit}
          onEnrollSample={context.onEnrollSample}
          onAnimationEnd={context.onAnimationEnd}
        />
      );
    },
    []
  );

  // Search integration
  const {
    isOpen: isSearchOpen,
    open: openSearch,
    close: closeSearch,
    matches: searchMatches,
    currentMatchIndex: searchMatchIndex,
  } = useSearchStore();
  const prevIsActiveRef = useRef(isActive);

  useEffect(() => {
    if (prevIsActiveRef.current && !isActive && isSearchOpen) {
      closeSearch();
    }
    prevIsActiveRef.current = isActive;
  }, [isActive, isSearchOpen, closeSearch]);

  useEffect(() => {
    if (!isActive) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === 'f') {
        const activeElement = document.activeElement;
        const isBodyFocus = activeElement === document.body;
        if (
          !isBodyFocus &&
          (!(activeElement instanceof HTMLElement) ||
            (!activeElement.closest('.projects-detail-pane') &&
              !activeElement.closest('.panel-right')))
        ) {
          return;
        }

        e.preventDefault();
        openSearch();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isActive, openSearch]);

  // Calculate total words/characters
  const wordCount = useMemo(() => {
    if (!segments || segments.length === 0) return 0;

    // Count CJK characters + words using Unicode properties
    let count = 0;
    for (const segment of segments) {
      if (!segment.text) continue;
      // \p{sc=Han} matches Chinese characters
      // \p{sc=Hiragana} matches Hiragana
      // \p{sc=Katakana} matches Katakana
      // \p{sc=Hangul} matches Korean Hangul
      // \p{L}+ matches a sequence of letters (words)
      // \d+ matches a sequence of numbers
      const matches = segment.text.match(
        /[\p{sc=Han}\p{sc=Hiragana}\p{sc=Katakana}\p{sc=Hangul}]|[\p{L}\d]+/gu
      );
      if (matches) {
        count += matches.length;
      }
    }
    return count;
  }, [segments]);

  const virtuosoComponents = useMemo(
    () => ({
      Header: TranscriptListHeader,
      Footer: TranscriptListFooter,
    }),
    []
  );

  // Scroll to active match
  useEffect(() => {
    if (isActive && isSearchOpen && searchMatches.length > 0 && searchMatchIndex >= 0) {
      const match = searchMatches[searchMatchIndex];
      const segmentIndex = segmentsRef.current.findIndex((s) => s.id === match.segmentId);

      if (segmentIndex !== -1 && virtuosoRef.current) {
        virtuosoRef.current.scrollToIndex({
          index: segmentIndex,
          align: 'center',
          behavior: 'smooth',
        });
      }
    }
  }, [isActive, isSearchOpen, searchMatchIndex, searchMatches]);

  if (segments.length === 0) {
    return (
      <div className="empty-state">
        <PlusCircleIcon />
        <p style={{ whiteSpace: 'pre-line' }}>{t('editor.empty_state')}</p>
      </div>
    );
  }

  return (
    <div className="transcript-editor" onWheel={onUserInteraction} onTouchMove={onUserInteraction}>
      <EditorToolbar />
      {(wordCount > 0 || detachMode !== null) && (
        <TranscriptScrollFollowBadge
          detachMode={detachMode}
          onResume={resumeFollow}
          wordCount={wordCount}
        />
      )}
      <TranscriptUIContext.Provider value={uiStore}>
        <Virtuoso<TranscriptSegment, TranscriptContext>
          ref={virtuosoRef}
          className="transcript-list"
          data={segments}
          context={contextValue}
          itemContent={itemContent}
          components={virtuosoComponents}
          atBottomStateChange={atBottomStateChange}
          atBottomThreshold={60}
        />
      </TranscriptUIContext.Provider>
      {isActive && <SearchUI />}
      <EnrollSpeakerSampleModal
        isOpen={Boolean(enrollTargetSegment)}
        onClose={() => setEnrollTargetSegment(null)}
        segment={enrollTargetSegment}
        audioPath={resolvedAudioPath}
      />
    </div>
  );
}

export default TranscriptEditor;
