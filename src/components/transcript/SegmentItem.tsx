import React, { useCallback, useContext, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useStore } from 'zustand';
import { useShallow } from 'zustand/react/shallow';
import { TranscriptSegment } from '../../types/transcript';
import { formatDisplayTime } from '../../utils/exportFormats';
import { EditIcon, TrashIcon, MergeIcon } from '../Icons';
import { SegmentTimestamp } from './SegmentTimestamp';
import { SegmentTokens } from './SegmentTokens';
import { TranscriptUIContext } from './TranscriptUIContext';
import { useSearchStore } from '../../stores/searchStore';
import { useTranscriptSessionStore } from '../../stores/transcriptSessionStore';
import { useTranscriptSidecarStore } from '../../stores/transcriptSidecarStore';
import { useConfigStore } from '../../stores/configStore';
import { useDialogStore } from '../../stores/dialogStore';
import { useProjectStore } from '../../stores/projectStore';
import {
    buildSpeakerCorrectionProfileSections,
    speakerCorrectionService,
} from '../../services/speakerCorrectionService';
import { SegmentEditor } from './SegmentEditor';

/** Props for SegmentItem component. */
export interface SegmentItemProps {
    segment: TranscriptSegment;
    index: number;
    showSpeakerLabel?: boolean;
    canMergeWithNext?: boolean;
    onSeek: (time: number) => void;
    onEdit: (id: string) => void;
    onSave: (id: string, text: string) => void;
    onDelete: (id: string) => void;
    onMergeWithNext: (id: string) => void;
    onSplit?: (id: string, leftText: string, rightText: string) => void;
    onAnimationEnd: (id: string) => void;
}



/**
 * Individual transcript segment item.
 * Supports viewing, seeking, editing, deleting, and merging.
 */
function SegmentItemComponent({
    segment,
    index,
    showSpeakerLabel = false,
    canMergeWithNext = true,
    onSeek,
    onEdit,
    onSave,
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

    const isActive = useStore(uiStore, useCallback((state) => state.activeSegmentId === segment.id, [segment.id]));
    const isEditing = useStore(uiStore, useCallback((state) => state.editingSegmentId === segment.id, [segment.id]));
    const isNew = useStore(uiStore, useCallback((state) => state.newSegmentIds.has(segment.id), [segment.id]));
    const isAligning = useStore(uiStore, useCallback((state) => state.aligningSegmentIds.has(segment.id), [segment.id]));

    // LLM state (translation visibility)
    const sourceHistoryId = useTranscriptSessionStore((state) => state.sourceHistoryId);
    const llmState = useTranscriptSidecarStore((state) => state.llmStates[sourceHistoryId || 'current']);
    const isTranslationVisible = llmState ? llmState.isTranslationVisible : false;
    const speakerProfiles = useConfigStore((state) => state.config.speakerProfiles);
    const activeProject = useProjectStore((state) => (
        state.activeProjectId
            ? state.projects.find((project) => project.id === state.activeProjectId) || null
            : null
    ));

    // Subscribe to store for hasNext to avoid passing unstable props
    const hasNext = useStore(uiStore, useCallback((state) => index < state.totalSegments - 1, [index]));

    // Search matches
    // Optimize: Select only what we need to avoid re-renders on every store change
    const matches = useSearchStore(useShallow(state =>
        state.matches.filter(m => m.segmentId === segment.id)
    ));
    const setActiveMatch = useSearchStore(useCallback(state => state.setActiveMatch, []));

    // Select active match only if it belongs to this segment
    // This prevents re-renders when the active match changes but is in a different segment
    const activeMatch = useSearchStore(useCallback((state) => {
        const match = state.matches[state.currentMatchIndex];
        return (match && match.segmentId === segment.id) ? match : null;
    }, [segment.id]));

    // Local state stores HTML for the editor
    const [isSpeakerMenuOpen, setIsSpeakerMenuOpen] = useState(false);
    const [showAllSpeakerProfiles, setShowAllSpeakerProfiles] = useState(false);
    const [isApplyingSpeakerProfile, setIsApplyingSpeakerProfile] = useState(false);
    const speakerMenuRef = useRef<HTMLDivElement>(null);
    const speakerProfileSections = useMemo(
        () => buildSpeakerCorrectionProfileSections(speakerProfiles, activeProject),
        [activeProject, speakerProfiles],
    );
    const hasSecondarySpeakerProfiles = speakerProfileSections.secondaryProfiles.length > 0;
    const speakerGroupId = segment.speakerAttribution?.groupId || segment.speaker?.id || '';
    const speakerCandidates = segment.speakerAttribution?.candidates || [];
    const canResetSpeakerGroup = Boolean(segment.speakerAttribution && segment.speakerAttribution.state !== 'anonymous');

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
        if (!isEditing) {
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
    ].filter(Boolean).join(' ');

    const speakerBadge = showSpeakerLabel && segment.speaker ? (
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
            {speakerBadge && (
                <div className="segment-speaker-row">
                    {speakerBadge}
                </div>
            )}

            <div className="transcript-segment-main">
                <SegmentTimestamp start={segment.start} onSeek={onSeek} />

                <div className="segment-content" onClick={handleTextClick} onDoubleClick={handleTextDoubleClick}>
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
                        />
                    )}
                    {isAligning && (
                        <span
                            className="segment-aligning-indicator"
                            data-tooltip={t('editor.aligning')}
                            aria-label={t('editor.aligning')}
                        />
                    )}
                    {isTranslationVisible && typeof segment.translation === 'string' && segment.translation && !isEditing && (
                        <div className="segment-translation" style={{ marginTop: '4px', color: 'var(--color-text-secondary)', fontSize: '0.9em' }}>
                            {segment.translation}
                        </div>
                    )}
                </div>

                <div className="segment-actions">
                    <button
                        className="btn btn-icon"
                        onClick={(e) => {
                            e.stopPropagation();
                            onEdit(segment.id);
                        }}
                        data-tooltip={t('editor.edit_tooltip')}
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
                            disabled={!canMergeWithNext}
                            data-tooltip={t('editor.merge_tooltip')}
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
                        data-tooltip={t('editor.delete_tooltip')}
                        aria-label={t('editor.delete_label', { time: formatDisplayTime(segment.start) })}
                    >
                        <TrashIcon />
                    </button>
                </div>
            </div>
        </div>
    );
}

export const SegmentItem = React.memo(SegmentItemComponent);
