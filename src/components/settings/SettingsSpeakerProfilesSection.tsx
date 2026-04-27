import React, { useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { remove } from '@tauri-apps/plugin-fs';
import { useTranslation } from 'react-i18next';
import { ChevronDown, ChevronRight, Mic, Plus, Trash2, Upload } from 'lucide-react';
import { v4 as uuidv4 } from 'uuid';
import { useVocabularyConfig, useSetConfig } from '../../stores/configStore';
import { useDialogStore } from '../../stores/dialogStore';
import { useProjectStore } from '../../stores/projectStore';
import { type ProjectDefaults } from '../../types/project';
import {
  normalizeSpeakerProfiles,
  type SpeakerProfile,
  type SpeakerProfileSample,
} from '../../types/speaker';
import { speakerService } from '../../services/speakerService';
import { SettingsSection } from './SettingsLayout';
import { Switch } from '../Switch';

function formatSampleDuration(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) {
    return '0.0s';
  }

  if (seconds >= 60) {
    const minutes = Math.floor(seconds / 60);
    const remain = seconds % 60;
    return `${minutes}m ${remain.toFixed(1)}s`;
  }

  return `${seconds.toFixed(1)}s`;
}

export function SettingsSpeakerProfilesSection(): React.JSX.Element {
  const { t } = useTranslation();
  const config = useVocabularyConfig();
  const updateConfig = useSetConfig();
  const showError = useDialogStore((state) => state.showError);
  const projects = useProjectStore((state) => state.projects);
  const updateProjectDefaults = useProjectStore((state) => state.updateProjectDefaults);

  const [newProfileName, setNewProfileName] = useState('');
  const [expandedProfileIds, setExpandedProfileIds] = useState<Set<string>>(new Set());
  const profiles = normalizeSpeakerProfiles(config.speakerProfiles);

  const persistProfiles = (nextProfiles: SpeakerProfile[]) => {
    updateConfig({ speakerProfiles: nextProfiles });
  };

  const toggleExpanded = (profileId: string) => {
    setExpandedProfileIds((previous) => {
      const next = new Set(previous);
      if (next.has(profileId)) {
        next.delete(profileId);
      } else {
        next.add(profileId);
      }
      return next;
    });
  };

  const removeProfileReferenceFromProjects = async (profileId: string) => {
    const affectedProjects = projects.filter((project) => (
      project.defaults.enabledSpeakerProfileIds.includes(profileId)
    ));
    if (affectedProjects.length === 0) {
      return;
    }

    await Promise.all(affectedProjects.map((project) => (
      updateProjectDefaults(project.id, {
        enabledSpeakerProfileIds: project.defaults.enabledSpeakerProfileIds.filter((id) => id !== profileId),
      } as Pick<ProjectDefaults, 'enabledSpeakerProfileIds'>)
    )));
  };

  const handleAddProfile = () => {
    const name = newProfileName.trim();
    if (!name) {
      return;
    }

    const newProfile: SpeakerProfile = {
      id: uuidv4(),
      name,
      enabled: true,
      samples: [],
    };

    persistProfiles([...profiles, newProfile]);
    setNewProfileName('');
    setExpandedProfileIds((previous) => new Set(previous).add(newProfile.id));
  };

  const handleUpdateProfile = (profileId: string, updates: Partial<SpeakerProfile>) => {
    persistProfiles(
      profiles.map((profile) => (
        profile.id === profileId ? { ...profile, ...updates } : profile
      )),
    );
  };

  const handleDeleteProfile = async (profile: SpeakerProfile) => {
    persistProfiles(profiles.filter((item) => item.id !== profile.id));
    setExpandedProfileIds((previous) => {
      const next = new Set(previous);
      next.delete(profile.id);
      return next;
    });

    await Promise.allSettled(profile.samples.map((sample) => remove(sample.filePath)));
    await removeProfileReferenceFromProjects(profile.id);
  };

  const handleDeleteSample = async (profileId: string, sampleId: string) => {
    const profile = profiles.find((item) => item.id === profileId);
    const sample = profile?.samples.find((item) => item.id === sampleId);
    if (!profile || !sample) {
      return;
    }

    persistProfiles(
      profiles.map((item) => (
        item.id === profileId
          ? { ...item, samples: item.samples.filter((entry) => entry.id !== sampleId) }
          : item
      )),
    );

    try {
      await remove(sample.filePath);
    } catch {
      // Ignore already-removed sample files.
    }
  };

  const handleImportSamples = async (profile: SpeakerProfile) => {
    try {
      const selected = await open({
        multiple: true,
        directory: false,
        filters: [{
          name: t('settings.audio_files', { defaultValue: 'Audio Files' }),
          extensions: ['wav', 'mp3', 'm4a', 'aac', 'ogg', 'flac', 'mp4', 'webm'],
        }],
      });

      if (!selected) {
        return;
      }

      const filePaths = Array.isArray(selected) ? selected : [selected];
      const importedSamples = await Promise.all(
        filePaths.map((filePath) => speakerService.importProfileSample(profile.id, filePath)),
      );

      const nextProfiles = profiles.map((item) => (
        item.id === profile.id
          ? {
              ...item,
              samples: [...item.samples, ...importedSamples]
                .reduce<SpeakerProfileSample[]>((accumulator, sample) => {
                  if (!accumulator.some((entry) => entry.id === sample.id)) {
                    accumulator.push(sample);
                  }
                  return accumulator;
                }, []),
            }
          : item
      ));

      persistProfiles(nextProfiles);
      setExpandedProfileIds((previous) => new Set(previous).add(profile.id));
    } catch (error) {
      await showError({
        code: 'speaker_profile.import_failed',
        messageKey: 'settings.speaker_profile_import_failed',
        messageParams: {
          defaultValue: 'Failed to import one or more speaker reference samples.',
        },
        cause: error,
      });
    }
  };

  return (
    <SettingsSection
      title={t('settings.speaker_profiles_title', { defaultValue: 'Speaker Profiles' })}
      icon={<Mic size={20} />}
      description={t('settings.speaker_profiles_description', {
        defaultValue: 'Build a global library of known speakers from local reference audio files. Projects can then choose which profiles are active.',
      })}
    >
      <div style={{
        display: 'flex',
        gap: '12px',
        padding: '24px',
        background: 'var(--color-bg-primary)',
        alignItems: 'flex-end',
        borderBottom: '1px solid var(--color-border-subtle)',
      }}>
        <div style={{ flex: 1 }}>
          <label style={{ display: 'block', fontSize: '0.85rem', marginBottom: '4px', color: 'var(--color-text-muted)' }}>
            {t('settings.speaker_profile_name_label', { defaultValue: 'Profile Name' })}
          </label>
          <input
            type="text"
            className="settings-input"
            value={newProfileName}
            onChange={(event) => setNewProfileName(event.target.value)}
            placeholder={t('settings.speaker_profile_name_placeholder', { defaultValue: 'e.g. Alice' })}
            style={{ width: '100%' }}
          />
        </div>
        <button
          className="btn btn-primary"
          onClick={handleAddProfile}
          disabled={!newProfileName.trim()}
          style={{ height: '38px', display: 'flex', alignItems: 'center', gap: '6px', padding: '0 20px' }}
        >
          <Plus size={18} />
          {t('settings.add_speaker_profile', { defaultValue: 'Add Profile' })}
        </button>
      </div>

      <div className="settings-list" style={{ background: 'var(--color-bg-primary)', overflow: 'hidden' }}>
        {profiles.length === 0 ? (
          <div style={{
            padding: '48px 24px',
            textAlign: 'center',
            color: 'var(--color-text-muted)',
          }}>
            {t('settings.no_speaker_profiles', { defaultValue: 'No speaker profiles yet.' })}
          </div>
        ) : (
          profiles.map((profile, index) => (
            <div
              key={profile.id}
              style={{
                borderBottom: index === profiles.length - 1 ? 'none' : '1px solid var(--color-border-subtle)',
                background: profile.enabled ? 'transparent' : 'var(--color-bg-secondary-soft)',
              }}
            >
              <div
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: '12px',
                  padding: '16px 24px',
                  cursor: 'pointer',
                }}
                onClick={() => toggleExpanded(profile.id)}
              >
                <div style={{ display: 'flex', alignItems: 'center', color: 'var(--color-text-muted)' }}>
                  {expandedProfileIds.has(profile.id) ? <ChevronDown size={20} /> : <ChevronRight size={20} />}
                </div>

                <div style={{ flex: 1, display: 'flex', alignItems: 'center', gap: '12px' }}>
                  <input
                    type="text"
                    className="settings-input-minimal"
                    value={profile.name}
                    onClick={(event) => event.stopPropagation()}
                    onChange={(event) => handleUpdateProfile(profile.id, { name: event.target.value })}
                    style={{ fontWeight: 600, fontSize: '1rem', width: 'auto', minWidth: '150px' }}
                  />
                  <span style={{ fontSize: '0.75rem', color: 'var(--color-text-muted)', background: 'var(--color-bg-secondary)', padding: '2px 8px', borderRadius: '10px' }}>
                    {t('settings.speaker_samples_count', {
                      count: profile.samples.length,
                      defaultValue: `${profile.samples.length} samples`,
                    })}
                  </span>
                </div>

                <div style={{ display: 'flex', alignItems: 'center', gap: '20px' }} onClick={(event) => event.stopPropagation()}>
                  <Switch
                    checked={profile.enabled}
                    onChange={(checked) => handleUpdateProfile(profile.id, { enabled: checked })}
                  />

                  <button
                    className="btn btn-icon btn-danger-soft"
                    onClick={() => void handleDeleteProfile(profile)}
                    title={t('settings.delete_speaker_profile', { defaultValue: `Delete ${profile.name}` })}
                    aria-label={t('settings.delete_speaker_profile', { defaultValue: `Delete ${profile.name}` })}
                  >
                    <Trash2 size={18} />
                  </button>
                </div>
              </div>

              {expandedProfileIds.has(profile.id) && (
                <div style={{
                  padding: '0 24px 24px 56px',
                  display: 'flex',
                  flexDirection: 'column',
                  gap: '12px',
                }}>
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: '12px', flexWrap: 'wrap' }}>
                    <p style={{ margin: 0, color: 'var(--color-text-muted)', fontSize: '0.85rem' }}>
                      {t('settings.speaker_profile_samples_hint', {
                        defaultValue: 'Import one or more local reference clips. They will be normalized to 16k mono WAV and stored under app-managed data.',
                      })}
                    </p>
                    <button
                      className="btn btn-secondary-soft"
                      onClick={() => void handleImportSamples(profile)}
                      style={{ display: 'flex', alignItems: 'center', gap: '6px' }}
                    >
                      <Upload size={16} />
                      {t('settings.import_speaker_samples', { defaultValue: 'Import Samples' })}
                    </button>
                  </div>

                  {profile.samples.length === 0 ? (
                    <div style={{ color: 'var(--color-text-muted)', fontSize: '0.85rem' }}>
                      {t('settings.no_speaker_samples', { defaultValue: 'No reference samples imported yet.' })}
                    </div>
                  ) : (
                    profile.samples.map((sample) => (
                      <div
                        key={sample.id}
                        style={{
                          display: 'flex',
                          alignItems: 'center',
                          gap: '12px',
                          padding: '10px 12px',
                          background: 'var(--color-bg-secondary)',
                          borderRadius: 'var(--radius-md)',
                        }}
                      >
                        <div style={{ flex: 1, minWidth: 0 }}>
                          <div style={{ fontWeight: 500 }}>{sample.sourceName}</div>
                          <div style={{ color: 'var(--color-text-muted)', fontSize: '0.8rem' }}>
                            {formatSampleDuration(sample.durationSeconds)}
                          </div>
                        </div>

                        <button
                          className="btn btn-icon btn-danger-soft"
                          onClick={() => void handleDeleteSample(profile.id, sample.id)}
                          aria-label={t('common.delete')}
                        >
                          <Trash2 size={14} />
                        </button>
                      </div>
                    ))
                  )}
                </div>
              )}
            </div>
          ))
        )}
      </div>
    </SettingsSection>
  );
}
