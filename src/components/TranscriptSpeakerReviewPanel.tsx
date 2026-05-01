import React, { useMemo, useState } from 'react';
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
  buildSpeakerReviewCounts,
  buildSpeakerReviewGroups,
  filterSpeakerReviewGroups,
  type SpeakerReviewFilter,
  type SpeakerReviewGroup,
} from '../services/speakerReviewService';
import './PanelModal.css';
import './TranscriptSpeakerReviewPanel.css';

interface TranscriptSpeakerReviewPanelProps {
  isOpen: boolean;
  onClose: () => void;
}

interface SpeakerReviewFilterOption {
  id: SpeakerReviewFilter;
  labelKey: string;
  countKey: keyof ReturnType<typeof buildSpeakerReviewCounts>;
}

const FILTER_OPTIONS: SpeakerReviewFilterOption[] = [
  { id: 'pending', labelKey: 'editor.speaker_review_filter_pending', countKey: 'pending' },
  { id: 'suggested', labelKey: 'editor.speaker_review_filter_suggested', countKey: 'suggested' },
  { id: 'anonymous', labelKey: 'editor.speaker_review_filter_anonymous', countKey: 'anonymous' },
  { id: 'identified', labelKey: 'editor.speaker_review_filter_identified', countKey: 'identified' },
  { id: 'reviewed', labelKey: 'editor.speaker_review_filter_reviewed', countKey: 'reviewed' },
  { id: 'all', labelKey: 'editor.speaker_review_filter_all', countKey: 'total' },
];

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

export function TranscriptSpeakerReviewPanel({
  isOpen,
  onClose,
}: TranscriptSpeakerReviewPanelProps): React.JSX.Element | null {
  const { t } = useTranslation();
  const showError = useDialogStore((state) => state.showError);
  const segments = useTranscriptSessionStore((state) => state.segments);
  const speakerProfiles = useConfigStore((state) => state.config.speakerProfiles);
  const requestSeek = useTranscriptPlaybackStore((state) => state.requestSeek);
  const activeProject = useProjectStore((state) => (
    state.activeProjectId
      ? state.projects.find((project) => project.id === state.activeProjectId) || null
      : null
  ));
  const groups = useMemo(() => buildSpeakerReviewGroups(segments), [segments]);
  const counts = useMemo(() => buildSpeakerReviewCounts(groups), [groups]);
  const profileSections = useMemo(
    () => buildSpeakerCorrectionProfileSections(speakerProfiles, activeProject),
    [activeProject, speakerProfiles],
  );
  const [activeFilter, setActiveFilter] = useState<SpeakerReviewFilter>('pending');
  const [expandedGroupIds, setExpandedGroupIds] = useState<Set<string>>(new Set());
  const [busyGroupId, setBusyGroupId] = useState<string | null>(null);
  const visibleGroups = useMemo(
    () => filterSpeakerReviewGroups(groups, activeFilter),
    [activeFilter, groups],
  );

  if (!isOpen) {
    return null;
  }

  const runGroupAction = async (
    groupId: string,
    action: () => Promise<unknown>,
    errorCode: string,
  ) => {
    try {
      setBusyGroupId(groupId);
      await action();
    } catch (error) {
      await showError({
        code: errorCode,
        messageKey: 'editor.speaker_correction_failed',
        cause: error,
      });
    } finally {
      setBusyGroupId(null);
    }
  };

  const handleConfirmGroup = async (groupId: string) => {
    await runGroupAction(
      groupId,
      () => speakerCorrectionService.confirmSpeakerGroupReview(groupId),
      'speaker_review.confirm_failed',
    );
  };

  const handleAssignProfile = async (groupId: string, profileId: string) => {
    await runGroupAction(
      groupId,
      () => speakerCorrectionService.assignProfileToSpeakerGroup(groupId, profileId),
      'speaker_review.apply_failed',
    );
  };

  const handleResetGroup = async (groupId: string) => {
    await runGroupAction(
      groupId,
      () => speakerCorrectionService.resetGroupToAnonymous(groupId),
      'speaker_review.reset_failed',
    );
  };

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
            {FILTER_OPTIONS.map((option) => (
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
            {visibleGroups.length === 0 ? (
              <div className="transcript-speaker-review-empty">
                <CheckCircle2 size={18} />
                {t('editor.speaker_review_empty')}
              </div>
            ) : visibleGroups.map((group) => {
              const isBusy = busyGroupId === group.groupId;
              const showAllProfiles = expandedGroupIds.has(group.groupId);
              const topCandidate = group.candidates[0];
              const canReset = group.state !== 'anonymous';

              return (
                <article
                  key={group.groupId}
                  className={`transcript-speaker-review-card is-${group.reviewStatus}`}
                  data-testid={`speaker-review-group-${group.groupId}`}
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
                      onClick={() => {
                        requestSeek(group.firstStart);
                        onClose();
                      }}
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
