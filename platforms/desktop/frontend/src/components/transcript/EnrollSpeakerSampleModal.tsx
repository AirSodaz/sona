import { Clock, Loader2, Mic, Plus, User } from 'lucide-react';
import type React from 'react';
import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { v4 as uuidv4 } from 'uuid';
import { speakerCorrectionService } from '../../services/speakerCorrectionService';
import { speakerService } from '../../services/speakerService';
import { useConfigStore } from '../../stores/configStore';
import { useDialogStore } from '../../stores/dialogStore';
import type { SpeakerProfile } from '../../types/speaker';
import { normalizeSpeakerProfiles } from '../../types/speakerNormalization';
import type { TranscriptSegment } from '../../types/transcript';
import { formatDisplayTime } from '../../utils/exportFormats';
import { FormField } from '../FormField';
import { Modal } from '../Modal';
import './EnrollSpeakerSampleModal.css';

export interface EnrollSpeakerSampleModalProps {
  isOpen: boolean;
  onClose: () => void;
  segment: TranscriptSegment | null;
  audioPath: string | null;
}

export function EnrollSpeakerSampleModal({
  isOpen,
  onClose,
  segment,
  audioPath,
}: EnrollSpeakerSampleModalProps): React.JSX.Element | null {
  const { t } = useTranslation();
  const rawProfiles = useConfigStore((state) => state.config.speakerProfiles);
  const setConfig = useConfigStore((state) => state.setConfig);
  const alert = useDialogStore((state) => state.alert);

  const profiles = useMemo(() => normalizeSpeakerProfiles(rawProfiles), [rawProfiles]);
  const hasExistingProfiles = profiles.length > 0;

  const [mode, setMode] = useState<'existing' | 'new'>(() =>
    hasExistingProfiles ? 'existing' : 'new'
  );
  const [selectedProfileId, setSelectedProfileId] = useState<string>('');
  const [newProfileName, setNewProfileName] = useState<string>('');
  const [sampleName, setSampleName] = useState<string>('');
  const [isSubmitting, setIsSubmitting] = useState<boolean>(false);
  const [errorBanner, setErrorBanner] = useState<string | null>(null);

  useEffect(() => {
    if (!isOpen || !segment) {
      return;
    }

    setSampleName(`${formatDisplayTime(segment.start)} - ${formatDisplayTime(segment.end)}`);
    setErrorBanner(null);

    const matchingProfile = profiles.find(
      (p) =>
        p.id === segment.speaker?.id ||
        p.name.toLowerCase() === (segment.speaker?.label || '').toLowerCase()
    );

    if (matchingProfile) {
      setMode('existing');
      setSelectedProfileId(matchingProfile.id);
      setNewProfileName('');
    } else if (hasExistingProfiles) {
      setMode('existing');
      setSelectedProfileId(profiles[0].id);
      setNewProfileName(segment.speaker?.label || '');
    } else {
      setMode('new');
      setSelectedProfileId('');
      setNewProfileName(segment.speaker?.label || '');
    }
  }, [isOpen, segment, profiles, hasExistingProfiles]);

  if (!isOpen || !segment) {
    return null;
  }

  const durationSec = Math.max(0, segment.end - segment.start);

  const handleSubmit = async (e?: React.FormEvent) => {
    if (e) {
      e.preventDefault();
    }
    if (isSubmitting) return;

    if (!audioPath) {
      setErrorBanner(
        t('editor.enroll_speaker_no_audio', {
          defaultValue: '当前会话未找到音频文件，无法提取声纹',
        })
      );
      return;
    }

    let targetProfileId = selectedProfileId;
    let targetProfileName = '';

    if (mode === 'new') {
      const trimmed = newProfileName.trim();
      if (!trimmed) {
        setErrorBanner(
          t('editor.enroll_speaker_name_required', {
            defaultValue: '请输入说话人姓名',
          })
        );
        return;
      }
      targetProfileId = uuidv4();
      targetProfileName = trimmed;
    } else {
      const found = profiles.find((p) => p.id === targetProfileId);
      if (!found) {
        setErrorBanner(
          t('editor.enroll_speaker_select_required', {
            defaultValue: '请选择目标说话人',
          })
        );
        return;
      }
      targetProfileName = found.name;
    }

    try {
      setIsSubmitting(true);
      setErrorBanner(null);

      const sample = await speakerService.enrollProfileSampleFromAudio(
        targetProfileId,
        audioPath,
        segment.start,
        segment.end,
        sampleName.trim() || undefined
      );

      let nextProfiles: SpeakerProfile[];
      if (mode === 'new') {
        const newProfile: SpeakerProfile = {
          id: targetProfileId,
          name: targetProfileName,
          enabled: true,
          samples: [sample],
        };
        nextProfiles = [...profiles, newProfile];
      } else {
        nextProfiles = profiles.map((p) =>
          p.id === targetProfileId ? { ...p, samples: [...p.samples, sample] } : p
        );
      }

      setConfig({ speakerProfiles: nextProfiles });

      if (segment.speakerAttribution?.groupId) {
        try {
          await speakerCorrectionService.assignProfileToSpeakerGroup(
            segment.speakerAttribution.groupId,
            targetProfileId
          );
        } catch (err) {
          console.warn('Failed to auto-assign enrolled speaker profile to group:', err);
        }
      }

      onClose();

      await alert(
        t('editor.enroll_speaker_success', {
          name: targetProfileName,
          defaultValue: `已成功将样本录入「${targetProfileName}」声纹库`,
        }),
        {
          variant: 'success',
          title: t('common.success', { defaultValue: 'Success' }),
        }
      );
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setErrorBanner(message);
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      size="sm"
      title={
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
          <Mic size={18} />
          <span>
            {t('editor.enroll_speaker_sample_title', {
              defaultValue: '录入说话人声纹样本',
            })}
          </span>
        </div>
      }
      footer={
        <>
          <button
            type="button"
            className="btn btn-secondary"
            onClick={onClose}
            disabled={isSubmitting}
          >
            {t('common.cancel', { defaultValue: '取消' })}
          </button>
          <button
            type="button"
            className="btn btn-primary"
            onClick={() => void handleSubmit()}
            disabled={isSubmitting || !audioPath}
          >
            {isSubmitting ? <Loader2 size={14} className="queue-icon-spin" /> : <Mic size={14} />}
            <span>{t('editor.enroll_speaker_submit', { defaultValue: '确定录入' })}</span>
          </button>
        </>
      }
    >
      <div className="enroll-speaker-preview-card">
        <div className="enroll-speaker-preview-header">
          <Clock size={12} />
          <span>
            {formatDisplayTime(segment.start)} - {formatDisplayTime(segment.end)}
          </span>
          <span className="enroll-speaker-duration-tag">{durationSec.toFixed(1)}s</span>
        </div>
        <div className="enroll-speaker-preview-quote">"{segment.text}"</div>
      </div>

      <form onSubmit={handleSubmit}>
        <FormField
          label={t('editor.enroll_speaker_target_profile', { defaultValue: '目标说话人' })}
        >
          <div className="enroll-speaker-segmented-tabs">
            <button
              type="button"
              className={`enroll-speaker-tab-btn ${mode === 'existing' ? 'active' : ''}`}
              disabled={!hasExistingProfiles}
              onClick={() => setMode('existing')}
            >
              <User size={13} />
              <span>{t('editor.enroll_speaker_use_existing', { defaultValue: '已有说话人' })}</span>
            </button>
            <button
              type="button"
              className={`enroll-speaker-tab-btn ${mode === 'new' ? 'active' : ''}`}
              onClick={() => setMode('new')}
            >
              <Plus size={13} />
              <span>{t('editor.enroll_speaker_create_new', { defaultValue: '新建说话人' })}</span>
            </button>
          </div>

          {mode === 'existing' ? (
            <select
              className="input"
              value={selectedProfileId}
              onChange={(e) => setSelectedProfileId(e.target.value)}
            >
              {profiles.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name} ({p.samples.length}{' '}
                  {t('editor.enroll_speaker_sample_count', { defaultValue: '个样本' })})
                </option>
              ))}
            </select>
          ) : (
            <input
              type="text"
              className="input"
              value={newProfileName}
              onChange={(e) => setNewProfileName(e.target.value)}
              placeholder={t('editor.enroll_speaker_new_name_placeholder', {
                defaultValue: '新说话人姓名',
              })}
              autoFocus
            />
          )}
        </FormField>

        <FormField
          label={t('editor.enroll_speaker_sample_name', { defaultValue: '样本备注（可选）' })}
        >
          <input
            type="text"
            className="input"
            value={sampleName}
            onChange={(e) => setSampleName(e.target.value)}
            placeholder={t('editor.enroll_speaker_sample_name_placeholder', {
              defaultValue: '例如：清晰发言片段',
            })}
          />
        </FormField>

        {(!audioPath || errorBanner) && (
          <div className="enroll-speaker-error-banner">
            {!audioPath
              ? t('editor.enroll_speaker_no_audio', {
                  defaultValue: '当前会话未找到音频文件，无法提取声纹',
                })
              : errorBanner}
          </div>
        )}
      </form>
    </Modal>
  );
}

export default EnrollSpeakerSampleModal;
