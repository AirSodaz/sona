import type React from 'react';
import { useCallback, useContext, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useStore } from 'zustand';
import { useShallow } from 'zustand/react/shallow';
import {
  buildSpeakerCorrectionProfileSections,
  speakerCorrectionService,
} from '../../services/speakerCorrectionService';
import { useConfigStore } from '../../stores/configStore';
import { useDialogStore } from '../../stores/dialogStore';
import { useSearchStore } from '../../stores/searchStore';
import { updateTranscriptSegment } from '../../stores/transcriptCoordinator';
import { useTranscriptSessionStore } from '../../stores/transcriptSessionStore';
import { DEFAULT_LLM_STATE } from '../../stores/transcriptSidecarState';
import { useTranscriptSidecarStore } from '../../stores/transcriptSidecarStore';
import type { TranscriptSegment } from '../../types/transcript';
import { formatDisplayTime } from '../../utils/exportFormats';
import { CheckIcon, CloseIcon, EditIcon, MergeIcon, TrashIcon } from '../Icons';
import { SegmentEditor } from './SegmentEditor';
import { SegmentTimestamp } from './SegmentTimestamp';
import { SegmentTokens } from './SegmentTokens';
import { TranscriptUIContext } from './TranscriptUIContext';

/** Props for SegmentItem component. */
export interface SegmentItemProps {
  segment: TranscriptSegment;
  index: number;
  showSpeakerLabel?: boolean;
  canMergeWithNext?: boolean;
  onSeek: (time: number) => void;
  onEdit: (id: string) => void;
  onSave: (id: string, text: string) => void;
  onSaveTranslation?: (id: string, translation: string) => void;
  onDelete: (id: string) => void;
  onMergeWithNext: (id: string) => void;
  onSplit?: (id: string, leftText: string, rightText: string) => void;
  onAnimationEnd: (id: string) => void;
}

/**
 * Individual transcript segment item.
 * Supports viewing, seeking, editing, deleting, and merging.
 */
export function SegmentItem({
  segment,
  index,
  showSpeakerLabel = false,
  canMergeWithNext = true,
  onSeek,
  onEdit,
  onSave,
  onSaveTranslation,
  onDelete,
  onMergeWithNext,
  onSplit,
  onAnimationEnd,
}: SegmentItemProps): React.JSX.Element {
  const { t } = useTranslation();
  const showError = useDialogStore((state) => state.showError);

  // Subscribe to UI state via context store to avoid parent re-renders and global store noise
  const uiStore = useContext(TranscriptUIContext);
  if (!uiStore) throw new Error('SegmentItem must be used within TranscriptUIContext');

  const isActive = useStore(
    uiStore,
    useCallback((state) => state.activeSegmentId === segment.id, [segment.id])
  );
  const isEditing = useStore(
    uiStore,
    useCallback((state) => state.editingSegmentId === segment.id, [segment.id])
  );
  const isNew = useStore(
    uiStore,
    useCallback((state) => state.newSegmentIds.has(segment.id), [segment.id])
  );
  const isAligning = useStore(
    uiStore,
    useCallback((state) => state.aligningSegmentIds.has(segment.id), [segment.id])
  );

  // LLM state (translation visibility)
  const sourceHistoryId = useTranscriptSessionStore((state) => state.sourceHistoryId);
  const llmState = useTranscriptSidecarStore(
    (state) => state.llmStates[sourceHistoryId || 'current']
  );
  const isTranslationVisible =
    llmState?.isTranslationVisible ?? DEFAULT_LLM_STATE.isTranslationVisible;
  const speakerProfiles = useConfigStore((state) => state.config.speakerProfiles);

  // Subscribe to store for hasNext to avoid passing unstable props
  const hasNext = useStore(
    uiStore,
    useCallback((state) => index < state.totalSegments - 1, [index])
  );

  const isLocked = !segment.isFinal;

  // Translation editing state
  const [isEditingTranslation, setIsEditingTranslation] = useState(false);
  const [translationDraft, setTranslationDraft] = useState('');
  const translationInputRef = useRef<HTMLTextAreaElement>(null);
  const isTranslationActionClickedRef = useRef(false);

  const adjustTextareaHeight = useCallback((el: HTMLTextAreaElement | null) => {
    if (!el) return;
    el.style.height = 'auto';
    el.style.height = `${Math.max(28, el.scrollHeight)}px`;
  }, []);

  const handleStartTranslationEdit = useCallback(() => {
    if (isLocked) return;
    setTranslationDraft(segment.translation || '');
    setIsEditingTranslation(true);
  }, [isLocked, segment.translation]);

  const handleSaveTranslation = useCallback(
    (text: string) => {
      const trimmed = text.trim();
      if (onSaveTranslation) {
        onSaveTranslation(segment.id, trimmed);
      } else {
        updateTranscriptSegment(segment.id, { translation: trimmed });
      }
      setIsEditingTranslation(false);
    },
    [onSaveTranslation, segment.id]
  );

  const handleCancelTranslation = useCallback(() => {
    setIsEditingTranslation(false);
    setTranslationDraft(segment.translation || '');
  }, [segment.translation]);

  useEffect(() => {
    if (isEditingTranslation && translationInputRef.current) {
      translationInputRef.current.focus();
      const len = translationInputRef.current.value.length;
      translationInputRef.current.setSelectionRange(len, len);
      adjustTextareaHeight(translationInputRef.current);
    }
  }, [isEditingTranslation, adjustTextareaHeight]);

  const handleTranslationKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        e.stopPropagation();
        handleSaveTranslation(translationDraft);
        return;
      }
      if (e.key === 'Escape') {
        e.preventDefault();
        e.stopPropagation();
        handleCancelTranslation();
        return;
      }
    },
    [handleSaveTranslation, handleCancelTranslation, translationDraft]
  );

  const handleTranslationBlur = useCallback(() => {
    if (isTranslationActionClickedRef.current) {
      return;
    }
    handleSaveTranslation(translationDraft);
  }, [handleSaveTranslation, translationDraft]);

  // Search matches
  // Optimize: Select only what we need to avoid re-renders on every store change
  const matches = useSearchStore(
    useShallow((state) => state.matches.filter((m) => m.segmentId === segment.id))
  );
  const setActiveMatch = useSearchStore(useCallback((state) => state.setActiveMatch, []));

  // Select active match only if it belongs to this segment
  // This prevents re-renders when the active match changes but is in a different segment
  const activeMatch = useSearchStore(
    useCallback(
      (state) => {
        const match = state.matches[state.currentMatchIndex];
        return match && match.segmentId === segment.id ? match : null;
      },
      [segment.id]
    )
  );

  // Local state stores HTML for the editor
  const [isSpeakerMenuOpen, setIsSpeakerMenuOpen] = useState(false);
  const [showAllSpeakerProfiles, setShowAllSpeakerProfiles] = useState(false);
  const [isApplyingSpeakerProfile, setIsApplyingSpeakerProfile] = useState(false);
  const speakerMenuRef = useRef<HTMLDivElement>(null);
  const speakerProfileSections = useMemo(
    () => buildSpeakerCorrectionProfileSections(speakerProfiles),
    [speakerProfiles]
  );
  const hasSecondarySpeakerProfiles = speakerProfileSections.secondaryProfiles.length > 0;
  const speakerGroupId = segment.speakerAttribution?.groupId || segment.speaker?.id || '';
  const speakerCandidates = segment.speakerAttribution?.candidates || [];
  const canResetSpeakerGroup = Boolean(
    segment.speakerAttribution && segment.speakerAttribution.state !== 'anonymous'
  );

  useEffect(() => {
    if (!isSpeakerMenuOpen) {
      return undefined;
    }

    const handlePointerDown = (event: MouseEvent) => {
      if (!speakerMenuRef.current?.contains(event.target as Node)) {
        setIsSpeakerMenuOpen(false);
        setShowAllSpeakerProfiles(false);
      }
    };

    document.addEventListener('mousedown', handlePointerDown);
    return () => document.removeEventListener('mousedown', handlePointerDown);
  }, [isSpeakerMenuOpen]);

  function handleTextClick(): void {
    if (!isEditing) {
      onSeek(segment.start);
    }
  }

  function handleTextDoubleClick(e: React.MouseEvent): void {
    if (!isEditing && !isLocked) {
      e.stopPropagation();
      onEdit(segment.id);
    }
  }

  function handleSave(text: string): void {
    onSave(segment.id, text);
  }

  function handleCancel(): void {
    onSave(segment.id, segment.text);
  }

  function handleSplitSegment(leftText: string, rightText: string): void {
    if (onSplit) {
      onSplit(segment.id, leftText, rightText);
    }
  }

  function closeSpeakerMenu(): void {
    setIsSpeakerMenuOpen(false);
    setShowAllSpeakerProfiles(false);
  }

  function handleSpeakerBadgeClick(e: React.MouseEvent): void {
    e.stopPropagation();
    setIsSpeakerMenuOpen((current) => {
      const next = !current;
      if (!next) {
        setShowAllSpeakerProfiles(false);
      }
      return next;
    });
  }

  async function handleSpeakerProfileSelect(profileId: string): Promise<void> {
    if (!speakerGroupId) {
      return;
    }

    try {
      setIsApplyingSpeakerProfile(true);
      await speakerCorrectionService.assignProfileToSpeakerGroup(speakerGroupId, profileId);
      closeSpeakerMenu();
    } catch (error) {
      await showError({
        code: 'speaker_correction.apply_failed',
        messageKey: 'editor.speaker_correction_failed',
        messageParams: {
          defaultValue: 'Failed to update speaker labels for this transcript.',
        },
        cause: error,
      });
    } finally {
      setIsApplyingSpeakerProfile(false);
    }
  }

  async function handleResetSpeakerGroup(): Promise<void> {
    if (!speakerGroupId) {
      return;
    }

    try {
      setIsApplyingSpeakerProfile(true);
      await speakerCorrectionService.resetGroupToAnonymous(speakerGroupId);
      closeSpeakerMenu();
    } catch (error) {
      await showError({
        code: 'speaker_correction.apply_failed',
        messageKey: 'editor.speaker_correction_failed',
        messageParams: {
          defaultValue: 'Failed to update speaker labels for this transcript.',
        },
        cause: error,
      });
    } finally {
      setIsApplyingSpeakerProfile(false);
    }
  }

  function handleAnimationEnd(e: React.AnimationEvent): void {
    // Only respond to our fade-in animation, not animations on child elements
    if (isNew && e.animationName === 'segmentFadeIn' && e.target === e.currentTarget) {
      onAnimationEnd(segment.id);
    }
  }

  const classNames = [
    'transcript-segment',
    isActive ? 'active' : '',
    isEditing ? 'editing' : '',
    isNew ? 'segment-new' : '',
  ]
    .filter(Boolean)
    .join(' ');

  const speakerBadge =
    showSpeakerLabel && segment.speaker ? (
      <div className="speaker-badge-shell" ref={speakerMenuRef}>
        <button
          type="button"
          className="speaker-badge-button"
          data-testid={`speaker-badge-${segment.id}`}
          aria-expanded={isSpeakerMenuOpen}
          aria-haspopup="menu"
          aria-label={t('editor.change_speaker_label', {
            speaker: segment.speaker.label,
            defaultValue: `Change speaker ${segment.speaker.label}`,
          })}
          onClick={handleSpeakerBadgeClick}
          disabled={isLocked}
          data-tooltip={isLocked ? t('editor.locked_not_final') : undefined}
        >
          {segment.speaker.label}
        </button>

        {isSpeakerMenuOpen && (
          <div
            className="speaker-correction-menu"
            data-testid={`speaker-correction-menu-${segment.id}`}
            role="menu"
            onClick={(event) => event.stopPropagation()}
          >
            <div className="speaker-correction-menu-header">
              <div className="speaker-correction-menu-title">
                {t('editor.speaker_correction_title', {
                  defaultValue: 'Assign speaker profile',
                })}
              </div>
              <div className="speaker-correction-menu-hint">
                {t('editor.speaker_correction_hint', {
                  defaultValue: 'Applies to every matching speaker segment in this transcript.',
                })}
              </div>
            </div>

            {speakerProfileSections.primaryProfiles.length === 0 && !hasSecondarySpeakerProfiles ? (
              <div className="speaker-correction-empty">
                {t('editor.speaker_correction_empty', {
                  defaultValue: 'No speaker profiles yet. Add them in Settings > Vocabulary.',
                })}
              </div>
            ) : (
              <>
                {speakerCandidates.length > 0 && (
                  <div className="speaker-correction-list">
                    {speakerCandidates.map((candidate) => (
                      <button
                        key={`${segment.id}-${candidate.profileId}`}
                        type="button"
                        className="speaker-correction-option"
                        role="menuitem"
                        disabled={isApplyingSpeakerProfile}
                        onClick={() => void handleSpeakerProfileSelect(candidate.profileId)}
                      >
                        {candidate.profileName}
                      </button>
                    ))}
                  </div>
                )}

                <div className="speaker-correction-list">
                  {speakerProfileSections.primaryProfiles.map((profile) => (
                    <button
                      key={profile.id}
                      type="button"
                      className="speaker-correction-option"
                      role="menuitem"
                      disabled={isApplyingSpeakerProfile}
                      onClick={() => void handleSpeakerProfileSelect(profile.id)}
                    >
                      {profile.name}
                    </button>
                  ))}
                </div>

                {canResetSpeakerGroup && (
                  <div className="speaker-correction-secondary">
                    <button
                      type="button"
                      className="speaker-correction-expand"
                      onClick={() => void handleResetSpeakerGroup()}
                    >
                      {t('editor.speaker_review_reset', {
                        defaultValue: 'Restore anonymous label',
                      })}
                    </button>
                  </div>
                )}

                {hasSecondarySpeakerProfiles && (
                  <div className="speaker-correction-secondary">
                    <button
                      type="button"
                      className="speaker-correction-expand"
                      data-testid={`speaker-correction-expand-${segment.id}`}
                      onClick={() => setShowAllSpeakerProfiles((current) => !current)}
                    >
                      {showAllSpeakerProfiles
                        ? t('editor.speaker_correction_hide_more', {
                            defaultValue: 'Hide more profiles',
                          })
                        : t('editor.speaker_correction_show_more', {
                            defaultValue: 'Show all speaker profiles',
                          })}
                    </button>

                    {showAllSpeakerProfiles && (
                      <div className="speaker-correction-list">
                        {speakerProfileSections.secondaryProfiles.map((profile) => (
                          <button
                            key={profile.id}
                            type="button"
                            className="speaker-correction-option secondary"
                            role="menuitem"
                            disabled={isApplyingSpeakerProfile}
                            onClick={() => void handleSpeakerProfileSelect(profile.id)}
                          >
                            {profile.name}
                          </button>
                        ))}
                      </div>
                    )}
                  </div>
                )}
              </>
            )}
          </div>
        )}
      </div>
    ) : null;

  return (
    <div className={classNames} onAnimationEnd={handleAnimationEnd}>
      {speakerBadge && <div className="segment-speaker-row">{speakerBadge}</div>}

      <div className="transcript-segment-main">
        <SegmentTimestamp start={segment.start} onSeek={onSeek} />

        <div
          className="segment-content"
          onClick={handleTextClick}
          onDoubleClick={handleTextDoubleClick}
        >
          {isEditing ? (
            <SegmentEditor
              segmentId={segment.id}
              initialHtml={segment.text}
              onSave={handleSave}
              onCancel={handleCancel}
              onSplit={handleSplitSegment}
            />
          ) : (
            <SegmentTokens
              segment={segment}
              isActive={isActive}
              onSeek={onSeek}
              matches={matches}
              activeMatch={activeMatch}
              onMatchClick={setActiveMatch}
              onEditTranslation={!isLocked ? handleStartTranslationEdit : undefined}
            />
          )}
          {isAligning && (
            <span
              className="segment-aligning-indicator"
              data-tooltip={t('editor.aligning')}
              aria-label={t('editor.aligning')}
            />
          )}
          {isTranslationVisible &&
            ((typeof segment.translation === 'string' && segment.translation.length > 0) ||
              isEditingTranslation) &&
            (isEditingTranslation ? (
              <div
                className="segment-translation-editor"
                onClick={(e) => e.stopPropagation()}
                onDoubleClick={(e) => e.stopPropagation()}
              >
                <textarea
                  ref={translationInputRef}
                  className="segment-translation-input"
                  value={translationDraft}
                  onChange={(e) => {
                    setTranslationDraft(e.target.value);
                    adjustTextareaHeight(e.target);
                  }}
                  onKeyDown={handleTranslationKeyDown}
                  onBlur={handleTranslationBlur}
                  rows={1}
                  placeholder={t('editor.translation_placeholder', {
                    defaultValue: 'Enter translation...',
                  })}
                />
                <div className="segment-translation-actions">
                  <button
                    type="button"
                    className="segment-translation-save-btn"
                    onMouseDown={(e) => {
                      e.preventDefault();
                      isTranslationActionClickedRef.current = true;
                    }}
                    onClick={(e) => {
                      e.stopPropagation();
                      handleSaveTranslation(translationDraft);
                      isTranslationActionClickedRef.current = false;
                    }}
                    title={t('common.save', { defaultValue: 'Save' })}
                    aria-label={t('common.save', { defaultValue: 'Save' })}
                  >
                    <CheckIcon width={14} height={14} />
                  </button>
                  <button
                    type="button"
                    className="segment-translation-cancel-btn"
                    onMouseDown={(e) => {
                      e.preventDefault();
                      isTranslationActionClickedRef.current = true;
                    }}
                    onClick={(e) => {
                      e.stopPropagation();
                      handleCancelTranslation();
                      isTranslationActionClickedRef.current = false;
                    }}
                    title={t('common.cancel', { defaultValue: 'Cancel' })}
                    aria-label={t('common.cancel', { defaultValue: 'Cancel' })}
                  >
                    <CloseIcon width={14} height={14} />
                  </button>
                </div>
              </div>
            ) : (
              <div
                className="segment-translation"
                onDoubleClick={(e) => {
                  e.stopPropagation();
                  if (!isLocked) {
                    handleStartTranslationEdit();
                  }
                }}
              >
                <span className="segment-translation-text">{segment.translation}</span>
                {!isLocked && (
                  <button
                    type="button"
                    className="segment-translation-edit-btn"
                    onClick={(e) => {
                      e.stopPropagation();
                      handleStartTranslationEdit();
                    }}
                    title={t('editor.edit_translation', { defaultValue: 'Edit translation' })}
                    aria-label={t('editor.edit_translation', {
                      defaultValue: 'Edit translation',
                    })}
                  >
                    <EditIcon width={12} height={12} />
                  </button>
                )}
              </div>
            ))}
        </div>

        <div className="segment-actions">
          <button
            className="btn btn-icon"
            onClick={(e) => {
              e.stopPropagation();
              onEdit(segment.id);
            }}
            disabled={isLocked}
            data-tooltip={isLocked ? t('editor.locked_not_final') : t('editor.edit_tooltip')}
            aria-label={t('editor.edit_label', { time: formatDisplayTime(segment.start) })}
          >
            <EditIcon />
          </button>
          {hasNext && (
            <button
              className="btn btn-icon"
              onClick={(e) => {
                e.stopPropagation();
                onMergeWithNext(segment.id);
              }}
              disabled={isLocked || !canMergeWithNext}
              data-tooltip={
                isLocked
                  ? t('editor.locked_not_final')
                  : !canMergeWithNext
                    ? t('editor.locked_merge_not_final')
                    : t('editor.merge_tooltip')
              }
              aria-label={t('editor.merge_label', { time: formatDisplayTime(segment.start) })}
            >
              <MergeIcon />
            </button>
          )}
          <button
            className="btn btn-icon"
            onClick={(e) => {
              e.stopPropagation();
              onDelete(segment.id);
            }}
            disabled={isLocked}
            data-tooltip={isLocked ? t('editor.locked_not_final') : t('editor.delete_tooltip')}
            aria-label={t('editor.delete_label', { time: formatDisplayTime(segment.start) })}
          >
            <TrashIcon />
          </button>
        </div>
      </div>
    </div>
  );
}
