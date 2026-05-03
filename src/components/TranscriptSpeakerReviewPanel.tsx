import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  CheckCircle2,
  ChevronDown,
  ChevronUp,
  Clock3,
  Loader2,
  MapPin,
  RotateCcw,
  UserCheck,
  Users,
  X,
} from 'lucide-react';
import { useDialogStore } from '../stores/dialogStore';
import { useConfigStore } from '../stores/configStore';
import { useProjectStore } from '../stores/projectStore';
import { useTranscriptPlaybackStore } from '../stores/transcriptPlaybackStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import {
  buildSpeakerCorrectionProfileSections,
  speakerCorrectionService,
} from '../services/speakerCorrectionService';
import {
  buildSpeakerReviewSnapshot,
  type SpeakerReviewCounts,
  type SpeakerReviewFilter,
  type SpeakerReviewFilterOption,
  type SpeakerReviewGroup,
  type SpeakerReviewSnapshot,
} from '../services/speakerReviewService';
import './PanelModal.css';
import './TranscriptSpeakerReviewPanel.css';

interface TranscriptSpeakerReviewPanelProps {
  isOpen: boolean;
  onClose: () => void;
}

const EMPTY_SPEAKER_REVIEW_COUNTS: SpeakerReviewCounts = {
  total: 0,
  pending: 0,
  suggested: 0,
  anonymous: 0,
  identified: 0,
  reviewed: 0,
};

function createEmptySpeakerReviewSnapshot(): SpeakerReviewSnapshot {
  return {
    groups: [],
    counts: EMPTY_SPEAKER_REVIEW_COUNTS,
    visibleGroups: [],
    filterOptions: [],
  };
}

function formatDuration(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) {
    return '0s';
  }
  if (seconds >= 60) {
    const minutes = Math.floor(seconds / 60);
    const remain = Math.round(seconds % 60);
    return `${minutes}m ${remain.toString().padStart(2, '0')}s`;
  }
  return `${Math.round(seconds)}s`;
}

function formatTimestamp(seconds: number): string {
  const safeSeconds = Math.max(0, Math.floor(seconds));
  const minutes = Math.floor(safeSeconds / 60);
  const remain = safeSeconds % 60;
  return `${minutes}:${remain.toString().padStart(2, '0')}`;
}

function getStatusKey(group: SpeakerReviewGroup): string {
  switch (group.reviewStatus) {
    case 'reviewed':
      return 'editor.speaker_review_status_reviewed';
    case 'auto':
      return 'editor.speaker_review_status_auto';
    case 'pending':
    default:
      return 'editor.speaker_review_status_pending';
  }
}

function getStateKey(group: SpeakerReviewGroup): string {
  switch (group.state) {
    case 'identified':
      return 'editor.speaker_review_state_identified';
    case 'suggested':
      return 'editor.speaker_review_state_suggested';
    case 'anonymous':
    default:
      return 'editor.speaker_review_state_anonymous';
  }
}

function getConfidenceKey(group: SpeakerReviewGroup): string {
  switch (group.confidence) {
    case 'high':
      return 'editor.speaker_review_confidence_high';
    case 'medium':
      return 'editor.speaker_review_confidence_medium';
    case 'low':
    default:
      return 'editor.speaker_review_confidence_low';
  }
}

function resolveNextActiveGroupId(groupId: string, groups: SpeakerReviewGroup[]): string | null {
  if (groups.length === 0) {
    return null;
  }

  const currentIndex = groups.findIndex((group) => group.groupId === groupId);
  if (currentIndex < 0) {
    return groups[0]?.groupId || null;
  }

  return groups[currentIndex + 1]?.groupId
    || groups[currentIndex - 1]?.groupId
    || null;
}

function isShortcutIgnoredTarget(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) {
    return false;
  }

  return Boolean(target.closest('input, textarea, select, button, [contenteditable="true"]'));
}

export function TranscriptSpeakerReviewPanel({
  isOpen,
  onClose,
}: TranscriptSpeakerReviewPanelProps): React.JSX.Element | null {
  const { t } = useTranslation();
  const modalRef = useRef<HTMLDivElement>(null);
  const busyGroupIdRef = useRef<string | null>(null);
  const snapshotRequestIdRef = useRef(0);
  const showError = useDialogStore((state) => state.showError);
  const segments = useTranscriptSessionStore((state) => state.segments);
  const speakerProfiles = useConfigStore((state) => state.config.speakerProfiles);
  const requestSeek = useTranscriptPlaybackStore((state) => state.requestSeek);
  const activeProject = useProjectStore((state) => (
    state.activeProjectId
      ? state.projects.find((project) => project.id === state.activeProjectId) || null
      : null
  ));
  const profileSections = useMemo(
    () => buildSpeakerCorrectionProfileSections(speakerProfiles, activeProject),
    [activeProject, speakerProfiles],
  );
  const [activeFilter, setActiveFilter] = useState<SpeakerReviewFilter>('pending');
  const [snapshot, setSnapshot] = useState<SpeakerReviewSnapshot>(() => createEmptySpeakerReviewSnapshot());
  const [isSnapshotLoading, setIsSnapshotLoading] = useState(false);
  const [expandedGroupIds, setExpandedGroupIds] = useState<Set<string>>(new Set());
  const [busyGroupId, setBusyGroupId] = useState<string | null>(null);
  const [activeGroupId, setActiveGroupId] = useState<string | null>(null);
  const counts = snapshot.counts;
  const visibleGroups = snapshot.visibleGroups;
  const filterOptions: SpeakerReviewFilterOption[] = snapshot.filterOptions;
  const effectiveActiveGroupId = (
    activeGroupId && visibleGroups.some((group) => group.groupId === activeGroupId)
      ? activeGroupId
      : visibleGroups[0]?.groupId || null
  );
  const activeGroup = useMemo(
    () => visibleGroups.find((group) => group.groupId === effectiveActiveGroupId) || null,
    [effectiveActiveGroupId, visibleGroups],
  );

  useEffect(() => {
    if (!isOpen) {
      snapshotRequestIdRef.current += 1;
      return undefined;
    }

    const requestId = snapshotRequestIdRef.current + 1;
    snapshotRequestIdRef.current = requestId;
    let cancelled = false;

    void Promise.resolve()
      .then(() => {
        if (!cancelled && snapshotRequestIdRef.current === requestId) {
          setIsSnapshotLoading(true);
        }
        return buildSpeakerReviewSnapshot(segments, activeFilter);
      })
      .then((nextSnapshot) => {
        if (cancelled || snapshotRequestIdRef.current !== requestId) {
          return;
        }

        setSnapshot(nextSnapshot);
        setActiveGroupId((current) => (
          current && nextSnapshot.visibleGroups.some((group) => group.groupId === current)
            ? current
            : nextSnapshot.visibleGroups[0]?.groupId || null
        ));
      })
      .catch(async (error) => {
        if (cancelled || snapshotRequestIdRef.current !== requestId) {
          return;
        }

        setSnapshot(createEmptySpeakerReviewSnapshot());
        await showError({
          code: 'speaker_review.snapshot_failed',
          messageKey: 'editor.speaker_correction_failed',
          cause: error,
        });
      })
      .finally(() => {
        if (!cancelled && snapshotRequestIdRef.current === requestId) {
          setIsSnapshotLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [activeFilter, isOpen, segments, showError]);

  const runGroupAction = useCallback(async (
    groupId: string,
    action: () => Promise<unknown>,
    errorCode: string,
  ) => {
    if (busyGroupIdRef.current) {
      return;
    }

    try {
      busyGroupIdRef.current = groupId;
      setBusyGroupId(groupId);
      await action();
      setActiveGroupId(resolveNextActiveGroupId(groupId, visibleGroups));
    } catch (error) {
      await showError({
        code: errorCode,
        messageKey: 'editor.speaker_correction_failed',
        cause: error,
      });
    } finally {
      busyGroupIdRef.current = null;
      setBusyGroupId(null);
    }
  }, [showError, visibleGroups]);

  const handleConfirmGroup = useCallback(async (groupId: string) => {
    await runGroupAction(
      groupId,
      () => speakerCorrectionService.confirmSpeakerGroupReview(groupId),
      'speaker_review.confirm_failed',
    );
  }, [runGroupAction]);

  const handleAssignProfile = useCallback(async (groupId: string, profileId: string) => {
    await runGroupAction(
      groupId,
      () => speakerCorrectionService.assignProfileToSpeakerGroup(groupId, profileId),
      'speaker_review.apply_failed',
    );
  }, [runGroupAction]);

  const handleResetGroup = useCallback(async (groupId: string) => {
    await runGroupAction(
      groupId,
      () => speakerCorrectionService.resetGroupToAnonymous(groupId),
      'speaker_review.reset_failed',
    );
  }, [runGroupAction]);

  const handleJumpToGroup = useCallback((group: SpeakerReviewGroup) => {
    requestSeek(group.firstStart);
    onClose();
  }, [onClose, requestSeek]);

  const moveActiveGroup = useCallback((direction: 1 | -1) => {
    setActiveGroupId((current) => {
      if (visibleGroups.length === 0) {
        return null;
      }

      const currentIndex = current
        ? visibleGroups.findIndex((group) => group.groupId === current)
        : -1;
      if (currentIndex < 0) {
        return direction > 0
          ? visibleGroups[0].groupId
          : visibleGroups[visibleGroups.length - 1].groupId;
      }

      const nextIndex = Math.min(
        visibleGroups.length - 1,
        Math.max(0, currentIndex + direction),
      );
      return visibleGroups[nextIndex].groupId;
    });
  }, [visibleGroups]);

  useEffect(() => {
    if (!isOpen) {
      return undefined;
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (
        event.target instanceof Node
        && modalRef.current
        && !modalRef.current.contains(event.target)
      ) {
        return;
      }

      if (event.defaultPrevented || isShortcutIgnoredTarget(event.target)) {
        return;
      }

      const key = event.key.toLowerCase();
      if (event.key === 'ArrowDown') {
        event.preventDefault();
        moveActiveGroup(1);
        return;
      }

      if (event.key === 'ArrowUp') {
        event.preventDefault();
        moveActiveGroup(-1);
        return;
      }

      if (!activeGroup || busyGroupIdRef.current) {
        return;
      }

      if (event.key === 'Enter') {
        event.preventDefault();
        void handleConfirmGroup(activeGroup.groupId);
        return;
      }

      if (key === 'a' && activeGroup.candidates[0]) {
        event.preventDefault();
        void handleAssignProfile(activeGroup.groupId, activeGroup.candidates[0].profileId);
        return;
      }

      if (key === 'r' && activeGroup.state !== 'anonymous') {
        event.preventDefault();
        void handleResetGroup(activeGroup.groupId);
        return;
      }

      if (key === 'j') {
        event.preventDefault();
        handleJumpToGroup(activeGroup);
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [
    activeGroup,
    handleAssignProfile,
    handleConfirmGroup,
    handleJumpToGroup,
    handleResetGroup,
    isOpen,
    moveActiveGroup,
  ]);

  if (!isOpen) {
    return null;
  }

  const toggleExpanded = (groupId: string) => {
    setExpandedGroupIds((current) => {
      const next = new Set(current);
      if (next.has(groupId)) {
        next.delete(groupId);
      } else {
        next.add(groupId);
      }
      return next;
    });
  };

  return (
    <div className="settings-overlay panel-modal-overlay transcript-speaker-review-overlay" onClick={onClose}>
      <div
        ref={modalRef}
        className="panel-modal-shell transcript-speaker-review-modal"
        onClick={(event) => event.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="speaker-review-title"
      >
        <div className="panel-modal-header transcript-speaker-review-header">
          <div className="panel-modal-header-copy">
            <div className="panel-modal-badge transcript-speaker-review-badge">
              <Users size={16} />
              <span>{t('editor.speaker_review_title')}</span>
            </div>
            <h2 id="speaker-review-title">{t('editor.speaker_review_title')}</h2>
            <p>{t('editor.speaker_review_description')}</p>
          </div>
          <div className="panel-modal-header-controls">
            <button
              type="button"
              className="btn btn-icon panel-modal-close"
              onClick={onClose}
              aria-label={t('common.close', { defaultValue: 'Close' })}
            >
              <X size={18} />
            </button>
          </div>
        </div>

        <div className="panel-modal-meta-row transcript-speaker-review-meta-row">
          <span className="panel-modal-meta-label">{t('editor.speaker_review_pending_count', { count: counts.pending })}</span>
          <span>{t('editor.speaker_review_reviewed_count', { count: counts.reviewed })}</span>
          <span>{t('editor.speaker_review_total_count', { count: counts.total })}</span>
        </div>

        <div className="panel-modal-content transcript-speaker-review-content">
          <div className="transcript-speaker-review-filters" aria-label={t('editor.speaker_review_title')}>
            {filterOptions.map((option) => (
              <button
                key={option.id}
                type="button"
                className={`transcript-speaker-review-filter ${activeFilter === option.id ? 'active' : ''}`}
                onClick={() => setActiveFilter(option.id)}
              >
                {t(option.labelKey, { count: counts[option.countKey] })}
              </button>
            ))}
          </div>

          <div className="transcript-speaker-review-list">
            {isSnapshotLoading && visibleGroups.length === 0 ? (
              <div className="transcript-speaker-review-empty">
                <Loader2 size={18} className="queue-icon-spin" />
                {t('common.loading', { defaultValue: 'Loading...' })}
              </div>
            ) : visibleGroups.length === 0 ? (
              <div className="transcript-speaker-review-empty">
                <CheckCircle2 size={18} />
                {t('editor.speaker_review_empty')}
              </div>
            ) : visibleGroups.map((group) => {
              const isBusy = busyGroupId === group.groupId;
              const isActive = effectiveActiveGroupId === group.groupId;
              const showAllProfiles = expandedGroupIds.has(group.groupId);
              const topCandidate = group.candidates[0];
              const canReset = group.state !== 'anonymous';

              return (
                <article
                  key={group.groupId}
                  className={`transcript-speaker-review-card is-${group.reviewStatus} ${isActive ? 'is-active' : ''}`}
                  data-testid={`speaker-review-group-${group.groupId}`}
                  aria-current={isActive ? 'true' : undefined}
                  onClick={() => setActiveGroupId(group.groupId)}
                >
                  <div className="transcript-speaker-review-card-head">
                    <div className="transcript-speaker-review-title-block">
                      <div className="transcript-speaker-review-speaker">
                        <span>{group.displayLabel}</span>
                        <span className="transcript-speaker-review-status">
                          {t(getStatusKey(group))}
                        </span>
                      </div>
                      <div className="transcript-speaker-review-tags">
                        <span>{t(getStateKey(group))}</span>
                        <span>{t(getConfidenceKey(group))}</span>
                        <span>{t('editor.speaker_review_segments_count', { count: group.segmentCount })}</span>
                        <span>{t('editor.speaker_review_duration', { duration: formatDuration(group.durationSeconds) })}</span>
                      </div>
                    </div>
                    <button
                      type="button"
                      className="btn btn-secondary btn-sm"
                      onClick={() => handleJumpToGroup(group)}
                    >
                      <MapPin size={14} />
                      {t('editor.speaker_review_jump')}
                    </button>
                  </div>

                  <div className="transcript-speaker-review-preview">
                    <div className="transcript-speaker-review-section-label">
                      <Clock3 size={13} />
                      {t('editor.speaker_review_preview_title')}
                    </div>
                    <div className="transcript-speaker-review-preview-list">
                      {group.previewSegments.map((preview) => (
                        <div key={preview.id} className="transcript-speaker-review-preview-row">
                          <span className="transcript-speaker-review-time">{formatTimestamp(preview.start)}</span>
                          <span className="transcript-speaker-review-text">{preview.text}</span>
                        </div>
                      ))}
                    </div>
                  </div>

                  <div className="transcript-speaker-review-candidates">
                    <div className="transcript-speaker-review-section-label">
                      <UserCheck size={13} />
                      {t('editor.speaker_review_candidates')}
                    </div>
                    {group.candidates.length > 0 ? (
                      <div className="transcript-speaker-review-candidate-list">
                        {group.candidates.map((candidate) => (
                          <span key={`${group.groupId}-${candidate.profileId}`} className="transcript-speaker-review-candidate">
                            {candidate.profileName} {candidate.score.toFixed(2)}
                          </span>
                        ))}
                      </div>
                    ) : (
                      <div className="transcript-speaker-review-muted">
                        {t('editor.speaker_review_no_candidates')}
                      </div>
                    )}
                  </div>

                  <div className="transcript-speaker-review-actions">
                    <button
                      type="button"
                      className="btn btn-secondary"
                      disabled={isBusy}
                      onClick={() => void handleConfirmGroup(group.groupId)}
                    >
                      {isBusy ? <Loader2 size={14} className="queue-icon-spin" /> : <CheckCircle2 size={14} />}
                      {t('editor.speaker_review_confirm')}
                    </button>
                    {topCandidate ? (
                      <button
                        type="button"
                        className="btn btn-primary"
                        disabled={isBusy}
                        onClick={() => void handleAssignProfile(group.groupId, topCandidate.profileId)}
                      >
                        <UserCheck size={14} />
                        {t('editor.speaker_review_apply_top_candidate', {
                          candidate: topCandidate.profileName,
                        })}
                      </button>
                    ) : null}
                    <button
                      type="button"
                      className="btn btn-secondary"
                      disabled={isBusy || !canReset}
                      onClick={() => void handleResetGroup(group.groupId)}
                    >
                      <RotateCcw size={14} />
                      {t('editor.speaker_review_reset')}
                    </button>
                  </div>

                  <div className="transcript-speaker-review-profile-block">
                    <div className="transcript-speaker-review-section-label">
                      {t('editor.speaker_review_assign_profile')}
                    </div>
                    <div className="transcript-speaker-review-profile-list">
                      {profileSections.primaryProfiles.map((profile) => (
                        <button
                          key={profile.id}
                          type="button"
                          className="btn btn-secondary-soft btn-sm"
                          disabled={isBusy}
                          onClick={() => void handleAssignProfile(group.groupId, profile.id)}
                        >
                          {profile.name}
                        </button>
                      ))}
                    </div>
                    {profileSections.secondaryProfiles.length > 0 ? (
                      <>
                        <button
                          type="button"
                          className="btn btn-secondary btn-sm transcript-speaker-review-expand"
                          onClick={() => toggleExpanded(group.groupId)}
                        >
                          {showAllProfiles ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
                          {showAllProfiles
                            ? t('editor.speaker_correction_hide_more')
                            : t('editor.speaker_correction_show_more')}
                        </button>
                        {showAllProfiles ? (
                          <div className="transcript-speaker-review-profile-list">
                            {profileSections.secondaryProfiles.map((profile) => (
                              <button
                                key={profile.id}
                                type="button"
                                className="btn btn-secondary-soft btn-sm"
                                disabled={isBusy}
                                onClick={() => void handleAssignProfile(group.groupId, profile.id)}
                              >
                                {profile.name}
                              </button>
                            ))}
                          </div>
                        ) : null}
                      </>
                    ) : null}
                  </div>
                </article>
              );
            })}
          </div>
        </div>
      </div>
    </div>
  );
}

export default TranscriptSpeakerReviewPanel;
