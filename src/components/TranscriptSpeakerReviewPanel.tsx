import React, { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useDialogStore } from '../stores/dialogStore';
import { useConfigStore } from '../stores/configStore';
import { useProjectStore } from '../stores/projectStore';
import { useTranscriptPlaybackStore } from '../stores/transcriptPlaybackStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import {
  buildSpeakerCorrectionProfileSections,
  speakerCorrectionService,
} from '../services/speakerCorrectionService';
import { buildSpeakerReviewGroups } from '../services/speakerReviewService';
import { XIcon } from './Icons';

interface TranscriptSpeakerReviewPanelProps {
  isOpen: boolean;
  onClose: () => void;
}

function formatDuration(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) {
    return '0s';
  }
  if (seconds >= 60) {
    const minutes = Math.floor(seconds / 60);
    const remain = Math.round(seconds % 60);
    return `${minutes}m ${remain}s`;
  }
  return `${Math.round(seconds)}s`;
}

function formatSourceLabel(source: 'auto' | 'manual'): string {
  return source === 'manual' ? 'Manual' : 'Automatic';
}

function formatConfidenceLabel(confidence: 'high' | 'medium' | 'low'): string {
  switch (confidence) {
    case 'high':
      return 'High';
    case 'medium':
      return 'Medium';
    case 'low':
    default:
      return 'Low';
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
  const profileSections = useMemo(
    () => buildSpeakerCorrectionProfileSections(speakerProfiles, activeProject),
    [activeProject, speakerProfiles],
  );
  const [expandedGroupIds, setExpandedGroupIds] = useState<Set<string>>(new Set());
  const [busyGroupId, setBusyGroupId] = useState<string | null>(null);

  if (!isOpen) {
    return null;
  }

  const handleAssignProfile = async (groupId: string, profileId: string) => {
    try {
      setBusyGroupId(groupId);
      await speakerCorrectionService.assignProfileToSpeakerGroup(groupId, profileId);
    } catch (error) {
      await showError({
        code: 'speaker_review.apply_failed',
        messageKey: 'editor.speaker_correction_failed',
        messageParams: {
          defaultValue: 'Failed to update speaker labels for this transcript.',
        },
        cause: error,
      });
    } finally {
      setBusyGroupId(null);
    }
  };

  const handleResetGroup = async (groupId: string) => {
    try {
      setBusyGroupId(groupId);
      await speakerCorrectionService.resetGroupToAnonymous(groupId);
    } catch (error) {
      await showError({
        code: 'speaker_review.reset_failed',
        messageKey: 'editor.speaker_correction_failed',
        messageParams: {
          defaultValue: 'Failed to update speaker labels for this transcript.',
        },
        cause: error,
      });
    } finally {
      setBusyGroupId(null);
    }
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
    <div
      className="transcript-summary-overlay"
      role="dialog"
      aria-modal="true"
      aria-label={t('editor.speaker_review_title', { defaultValue: 'Speaker Review' })}
    >
      <div className="transcript-summary-panel" style={{ maxWidth: '640px' }}>
        <div className="transcript-summary-header">
          <div>
            <div className="transcript-summary-title">
              {t('editor.speaker_review_title', { defaultValue: 'Speaker Review' })}
            </div>
            <div className="transcript-summary-subtitle">
              {t('editor.speaker_review_hint', { defaultValue: 'Review anonymous turns, low-confidence suggestions, and identified speakers in one place.' })}
            </div>
          </div>
          <button
            type="button"
            className="btn btn-icon btn-sm"
            aria-label={t('common.close', { defaultValue: 'Close' })}
            onClick={onClose}
          >
            <XIcon />
          </button>
        </div>

        <div className="transcript-summary-body" style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
          {groups.length === 0 ? (
            <div style={{ color: 'var(--color-text-muted)' }}>
              {t('editor.speaker_review_empty', { defaultValue: 'No speaker groups available in this transcript yet.' })}
            </div>
          ) : groups.map((group) => {
            const isBusy = busyGroupId === group.groupId;
            const showAllProfiles = expandedGroupIds.has(group.groupId);
            const topCandidate = group.candidates[0];
            return (
              <div
                key={group.groupId}
                data-testid={`speaker-review-group-${group.groupId}`}
                style={{
                  border: '1px solid var(--color-border-subtle)',
                  borderRadius: 'var(--radius-lg)',
                  padding: '12px',
                  display: 'flex',
                  flexDirection: 'column',
                  gap: '10px',
                }}
              >
                <div style={{ display: 'flex', justifyContent: 'space-between', gap: '12px', alignItems: 'flex-start' }}>
                  <div>
                    <div style={{ fontWeight: 600 }}>{group.displayLabel}</div>
                    <div style={{ color: 'var(--color-text-muted)', fontSize: '0.85rem' }}>
                      {`${group.segmentCount} segments · ${formatDuration(group.durationSeconds)} · ${group.state}`}
                    </div>
                    <div style={{ color: 'var(--color-text-muted)', fontSize: '0.8rem', marginTop: '4px' }}>
                      {t('editor.speaker_review_meta', {
                        defaultValue: `Source: ${formatSourceLabel(group.source)} · Confidence: ${formatConfidenceLabel(group.confidence)}`,
                      })}
                    </div>
                  </div>
                  <button
                    type="button"
                    className="btn btn-secondary"
                    onClick={() => {
                      requestSeek(group.firstStart);
                      onClose();
                    }}
                  >
                    {t('editor.speaker_review_jump', { defaultValue: 'Jump to first segment' })}
                  </button>
                </div>

                {topCandidate ? (
                  <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
                    <button
                      type="button"
                      className="btn btn-primary"
                      disabled={isBusy}
                      onClick={() => void handleAssignProfile(group.groupId, topCandidate.profileId)}
                    >
                      {t('editor.speaker_review_apply_top_candidate', {
                        candidate: topCandidate.profileName,
                        defaultValue: `Apply ${topCandidate.profileName}`,
                      })}
                    </button>
                    <div style={{ alignSelf: 'center', color: 'var(--color-text-muted)', fontSize: '0.85rem' }}>
                      {t('editor.speaker_review_candidate_score', {
                        score: topCandidate.score.toFixed(2),
                        defaultValue: `Top candidate score ${topCandidate.score.toFixed(2)}`,
                      })}
                    </div>
                  </div>
                ) : null}

                {group.candidates.length > 0 ? (
                  <div style={{ color: 'var(--color-text-muted)', fontSize: '0.85rem' }}>
                    {t('editor.speaker_review_candidates', { defaultValue: 'Candidates:' })}{' '}
                    {group.candidates
                      .map((candidate) => `${candidate.profileName} (${candidate.score.toFixed(2)})`)
                      .join(' · ')}
                  </div>
                ) : null}

                <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
                  {profileSections.primaryProfiles.map((profile) => (
                    <button
                      key={profile.id}
                      type="button"
                      className="btn btn-secondary-soft"
                      disabled={isBusy}
                      onClick={() => void handleAssignProfile(group.groupId, profile.id)}
                    >
                      {profile.name}
                    </button>
                  ))}
                  {group.state !== 'anonymous' ? (
                    <button
                      type="button"
                      className="btn btn-secondary"
                      disabled={isBusy}
                      onClick={() => void handleResetGroup(group.groupId)}
                    >
                      {t('editor.speaker_review_reset', { defaultValue: 'Restore anonymous label' })}
                    </button>
                  ) : null}
                </div>

                {profileSections.secondaryProfiles.length > 0 ? (
                  <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
                    <button
                      type="button"
                      className="btn btn-secondary"
                      onClick={() => toggleExpanded(group.groupId)}
                    >
                      {showAllProfiles
                        ? t('editor.speaker_correction_hide_more', { defaultValue: 'Hide more profiles' })
                        : t('editor.speaker_correction_show_more', { defaultValue: 'Show all speaker profiles' })}
                    </button>
                    {showAllProfiles ? (
                      <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
                        {profileSections.secondaryProfiles.map((profile) => (
                          <button
                            key={profile.id}
                            type="button"
                            className="btn btn-secondary-soft"
                            disabled={isBusy}
                            onClick={() => void handleAssignProfile(group.groupId, profile.id)}
                          >
                            {profile.name}
                          </button>
                        ))}
                      </div>
                    ) : null}
                  </div>
                ) : null}
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}

export default TranscriptSpeakerReviewPanel;
