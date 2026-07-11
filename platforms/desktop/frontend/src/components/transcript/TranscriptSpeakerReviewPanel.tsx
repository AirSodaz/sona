import React, { useRef } from 'react';
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
} from 'lucide-react';
import { type SpeakerReviewGroup } from '../../services/speakerReviewService';
import { PanelModal } from '../PanelModal';
import { useSpeakerReview } from '../../hooks/useSpeakerReview';
import './TranscriptSpeakerReviewPanel.css';

interface TranscriptSpeakerReviewPanelProps {
  isOpen: boolean;
  onClose: () => void;
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
  const modalRef = useRef<HTMLDivElement>(null);

  const {
    snapshot,
    activeFilter,
    isSnapshotLoading,
    expandedGroupIds,
    busyGroupId,
    effectiveActiveGroupId,
    profileSections,
    setActiveFilter,
    setActiveGroupId,
    toggleExpanded,
    handleConfirmGroup,
    handleAssignProfile,
    handleResetGroup,
    handleJumpToGroup,
  } = useSpeakerReview({ isOpen, onClose, modalRef });

  if (!isOpen) {
    return null;
  }

  const counts = snapshot.counts;
  const visibleGroups = snapshot.visibleGroups;
  const filterOptions = snapshot.filterOptions;

  return (
    <PanelModal
      isOpen={isOpen}
      onClose={onClose}
      ariaLabelledby="speaker-review-title"
      size="settings"
      className="transcript-speaker-review-modal"
      overlayClassName="transcript-speaker-review-overlay"
      shellRef={modalRef}
      badge={(
        <>
          <Users size={16} />
          <span>{t('editor.speaker_review_title')}</span>
        </>
      )}
      title={<h2 id="speaker-review-title">{t('editor.speaker_review_title')}</h2>}
      description={t('editor.speaker_review_description')}
      meta={(
        <>
          <span className="panel-modal-meta-label">{t('editor.speaker_review_pending_count', { count: counts.pending })}</span>
          <span>{t('editor.speaker_review_reviewed_count', { count: counts.reviewed })}</span>
          <span>{t('editor.speaker_review_total_count', { count: counts.total })}</span>
        </>
      )}
    >
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
                    <span>{t('editor.speaker_review_duration', { duration: group.displayDuration })}</span>
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
                      <span className="transcript-speaker-review-time">{preview.displayStart}</span>
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
                        {candidate.profileName} {candidate.displayScore}
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
    </PanelModal>
  );
}

export default TranscriptSpeakerReviewPanel;
