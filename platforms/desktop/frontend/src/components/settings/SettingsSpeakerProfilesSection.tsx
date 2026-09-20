import {
  ChevronDown,
  ChevronRight,
  Globe,
  Mic,
  Plus,
  Search,
  Trash2,
  Upload,
  X,
} from 'lucide-react';
import type React from 'react';
import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { v4 as uuidv4 } from 'uuid';
import { speakerService } from '../../services/speakerService';
import { openDialog } from '../../services/tauri/platform/dialog';
import { remove } from '../../services/tauri/platform/fs';
import { useAutomationStore } from '../../stores/automationStore';
import { useSetConfig, useVocabularyConfig } from '../../stores/configStore';
import { useDialogStore } from '../../stores/dialogStore';
import { useProjectStore } from '../../stores/projectStore';
import type { SpeakerProfile, SpeakerProfileSample } from '../../types/speaker';
import {
  deriveSpeakerProfileReadiness,
  normalizeSpeakerProfiles,
} from '../../types/speakerNormalization';
import { Dropdown } from '../Dropdown';
import { Modal } from '../Modal';
import { Switch } from '../Switch';
import { SettingsSection } from './SettingsLayout';
import './vocabulary/vocabulary.css';

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
  const removeProfileDependency = useAutomationStore((state) => state.removeProfileDependency);
  const projects = useProjectStore((state) => state.projects);

  const [newProfileName, setNewProfileName] = useState('');
  const [newProfileScope, setNewProfileScope] = useState<string>('global');
  const [scopeFilter, setScopeFilter] = useState<string>('all');
  const [searchQuery, setSearchQuery] = useState('');
  const [expandedProfileIds, setExpandedProfileIds] = useState<Set<string>>(new Set());

  // Modal for editing scope
  const [editingScopeProfile, setEditingScopeProfile] = useState<SpeakerProfile | null>(null);
  const [scopeModalType, setScopeModalType] = useState<'global' | 'project'>('global');
  const [scopeModalProjectIds, setScopeModalProjectIds] = useState<string[]>([]);

  const profiles = normalizeSpeakerProfiles(config.speakerProfiles);

  const persistProfiles = (nextProfiles: SpeakerProfile[]) => {
    updateConfig({ speakerProfiles: nextProfiles });
  };

  // Scope counts for filter pills
  const scopeCounts = useMemo(() => {
    const counts: Record<string, number> = {
      all: profiles.length,
      global: 0,
    };
    for (const p of projects) {
      counts[p.id] = 0;
    }
    for (const profile of profiles) {
      if ((profile.scope ?? 'global') === 'global') {
        counts.global++;
      } else if (profile.scope === 'project' && profile.projectIds) {
        for (const pid of profile.projectIds) {
          if (counts[pid] !== undefined) {
            counts[pid]++;
          }
        }
      }
    }
    return counts;
  }, [profiles, projects]);

  // Filtered profiles
  const filteredProfiles = useMemo(() => {
    return profiles.filter((profile) => {
      // Scope filter
      if (scopeFilter !== 'all') {
        const scope = profile.scope ?? 'global';
        if (scopeFilter === 'global') {
          if (scope !== 'global') return false;
        } else {
          if (scope !== 'project' || !profile.projectIds?.includes(scopeFilter)) {
            return false;
          }
        }
      }

      // Search query
      if (searchQuery.trim()) {
        const q = searchQuery.toLowerCase().trim();
        return profile.name.toLowerCase().includes(q);
      }

      return true;
    });
  }, [profiles, scopeFilter, searchQuery]);

  const scopeOptions = useMemo(() => {
    return [
      { value: 'global', label: t('settings.dict_scope_badge_global', { defaultValue: 'Global' }) },
      ...projects.map((p) => ({ value: p.id, label: p.name })),
    ];
  }, [projects, t]);
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

  const handleAddProfile = () => {
    const name = newProfileName.trim();
    if (!name) {
      return;
    }

    const isProjectScope = newProfileScope !== 'global';
    const newProfile: SpeakerProfile = {
      id: uuidv4(),
      name,
      enabled: true,
      scope: isProjectScope ? 'project' : 'global',
      projectIds: isProjectScope ? [newProfileScope] : [],
      samples: [],
    };

    persistProfiles([...profiles, newProfile]);
    setNewProfileName('');
    setExpandedProfileIds((previous) => new Set(previous).add(newProfile.id));
  };

  const openScopeModal = (profile: SpeakerProfile) => {
    setEditingScopeProfile(profile);
    setScopeModalType(profile.scope === 'project' ? 'project' : 'global');
    setScopeModalProjectIds(profile.projectIds ? [...profile.projectIds] : []);
  };

  const handleSaveScopeModal = () => {
    if (!editingScopeProfile) return;
    handleUpdateProfile(editingScopeProfile.id, {
      scope: scopeModalType,
      projectIds: scopeModalType === 'project' ? scopeModalProjectIds : [],
    });
    setEditingScopeProfile(null);
  };

  const getScopeBadgeText = (profile: SpeakerProfile): string => {
    if (profile.scope !== 'project') {
      return t('settings.dict_scope_badge_global', { defaultValue: 'Global' });
    }
    const pids = profile.projectIds ?? [];
    if (pids.length === 0) {
      return t('settings.speaker_scope_unassigned', { defaultValue: 'No Projects' });
    }
    if (pids.length === 1) {
      const proj = projects.find((p) => p.id === pids[0]);
      return proj ? proj.name : pids[0];
    }
    return t('settings.speaker_scope_projects_count', {
      count: pids.length,
      defaultValue: `${pids.length} Projects`,
    });
  };

  const handleUpdateProfile = (profileId: string, updates: Partial<SpeakerProfile>) => {
    persistProfiles(
      profiles.map((profile) => (profile.id === profileId ? { ...profile, ...updates } : profile))
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
    await removeProfileDependency('speakerProfile', profile.id);
  };

  const handleDeleteSample = async (profileId: string, sampleId: string) => {
    const profile = profiles.find((item) => item.id === profileId);
    const sample = profile?.samples.find((item) => item.id === sampleId);
    if (!profile || !sample) {
      return;
    }

    persistProfiles(
      profiles.map((item) =>
        item.id === profileId
          ? { ...item, samples: item.samples.filter((entry) => entry.id !== sampleId) }
          : item
      )
    );

    try {
      await remove(sample.filePath);
    } catch {
      // Ignore already-removed sample files.
    }
  };

  const handleImportSamples = async (profile: SpeakerProfile) => {
    try {
      const selected = await openDialog({
        multiple: true,
        directory: false,
        filters: [
          {
            name: t('settings.audio_files', { defaultValue: 'Audio Files' }),
            extensions: ['wav', 'mp3', 'm4a', 'aac', 'ogg', 'flac', 'mp4', 'webm'],
          },
        ],
      });

      if (!selected) {
        return;
      }

      const filePaths = Array.isArray(selected) ? selected : [selected];
      const importedSamples = await Promise.all(
        filePaths.map((filePath) => speakerService.importProfileSample(profile.id, filePath))
      );

      const nextProfiles = profiles.map((item) =>
        item.id === profile.id
          ? {
              ...item,
              samples: [...item.samples, ...importedSamples].reduce<SpeakerProfileSample[]>(
                (accumulator, sample) => {
                  if (!accumulator.some((entry) => entry.id === sample.id)) {
                    accumulator.push(sample);
                  }
                  return accumulator;
                },
                []
              ),
            }
          : item
      );

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
    <>
      <SettingsSection
        title={t('settings.speaker_profiles_title', { defaultValue: 'Speaker Profiles' })}
        icon={<Mic size={20} />}
        description={t('settings.speaker_profiles_description', {
          defaultValue:
            'Build a global library of known speakers from local reference audio files. Projects can then choose which profiles are active.',
        })}
      >
        {/* Scope Filter Pills & Search */}
        <div
          className="dict-filters"
          style={{
            padding: '16px 24px',
            borderBottom: '1px solid var(--color-border-subtle)',
            background: 'var(--color-bg-primary)',
          }}
        >
          <div className="dict-scope-pills">
            {[
              {
                id: 'all',
                label: t('settings.dict_scope_filter_all', { defaultValue: 'All Scopes' }),
                count: scopeCounts.all,
              },
              {
                id: 'global',
                label: t('settings.dict_scope_badge_global', { defaultValue: 'Global' }),
                count: scopeCounts.global,
              },
              ...projects.map((p) => ({
                id: p.id,
                label: p.name,
                count: scopeCounts[p.id] || 0,
              })),
            ].map((scope) => (
              <button
                key={scope.id}
                type="button"
                onClick={() => {
                  setScopeFilter(scope.id);
                  if (scope.id !== 'all') {
                    setNewProfileScope(scope.id);
                  } else {
                    setNewProfileScope('global');
                  }
                }}
                className="dict-scope-pill"
                aria-pressed={scopeFilter === scope.id}
              >
                <span>{scope.label}</span>
                <span className="dict-scope-pill-count">{scope.count}</span>
              </button>
            ))}
          </div>

          <div className="dict-search-wrap" style={{ minWidth: '200px' }}>
            <Search size={14} className="dict-search-icon" />
            <input
              type="text"
              className="settings-input"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder={t('settings.speaker_search_placeholder', {
                defaultValue: 'Search speakers...',
              })}
              style={{
                width: '100%',
                height: '32px',
                paddingLeft: '32px',
                paddingRight: searchQuery ? '28px' : '10px',
                fontSize: '0.8125rem',
                boxSizing: 'border-box',
              }}
            />
            {searchQuery && (
              <button
                type="button"
                className="dict-search-clear"
                onClick={() => setSearchQuery('')}
                aria-label={t('common.clear', { defaultValue: 'Clear' })}
              >
                <X size={12} />
              </button>
            )}
          </div>
        </div>

        {/* Add Profile Row */}
        <div
          style={{
            display: 'flex',
            gap: '12px',
            padding: '16px 24px',
            background: 'var(--color-bg-primary)',
            alignItems: 'flex-end',
            borderBottom: '1px solid var(--color-border-subtle)',
          }}
        >
          <div style={{ flex: 1 }}>
            <label
              style={{
                display: 'block',
                fontSize: '0.85rem',
                marginBottom: '4px',
                color: 'var(--color-text-muted)',
              }}
            >
              {t('settings.speaker_profile_name_label', { defaultValue: 'Profile Name' })}
            </label>
            <input
              type="text"
              className="settings-input"
              value={newProfileName}
              onChange={(event) => setNewProfileName(event.target.value)}
              placeholder={t('settings.speaker_profile_name_placeholder', {
                defaultValue: 'e.g. Alice',
              })}
              style={{ width: '100%' }}
            />
          </div>
          <div style={{ width: '160px' }}>
            <label
              style={{
                display: 'block',
                fontSize: '0.85rem',
                marginBottom: '4px',
                color: 'var(--color-text-muted)',
              }}
            >
              {t('settings.dict_target_scope', { defaultValue: 'Target Scope' })}
            </label>
            <Dropdown
              value={newProfileScope}
              onChange={(val) => setNewProfileScope(val)}
              options={scopeOptions}
              style={{ width: '100%' }}
              aria-label={t('settings.dict_target_scope', { defaultValue: 'Target Scope' })}
            />
          </div>
          <button
            className="btn btn-primary"
            onClick={handleAddProfile}
            disabled={!newProfileName.trim()}
            style={{
              height: '38px',
              display: 'flex',
              alignItems: 'center',
              gap: '6px',
              padding: '0 20px',
            }}
          >
            <Plus size={18} />
            {t('settings.add_speaker_profile', { defaultValue: 'Add Profile' })}
          </button>
        </div>

        <div
          className="settings-list"
          style={{ background: 'var(--color-bg-primary)', overflow: 'hidden' }}
        >
          {filteredProfiles.length === 0 ? (
            <div
              style={{
                padding: '48px 24px',
                textAlign: 'center',
                color: 'var(--color-text-muted)',
              }}
            >
              {t('settings.no_speaker_profiles', { defaultValue: 'No speaker profiles found.' })}
            </div>
          ) : (
            filteredProfiles.map((profile, index) => {
              const readiness = deriveSpeakerProfileReadiness(profile);
              const readinessCopy =
                readiness.state === 'ready'
                  ? t('settings.speaker_profile_readiness_ready', {
                      defaultValue: 'Ready for automatic matching',
                    })
                  : readiness.state === 'limited'
                    ? t('settings.speaker_profile_readiness_limited', {
                        defaultValue:
                          'Can appear as a suggestion, but needs more usable samples before automatic matching.',
                      })
                    : t('settings.speaker_profile_readiness_not_ready', {
                        defaultValue:
                          'Needs more usable samples before it can participate in speaker recognition.',
                      });

              return (
                <div
                  key={profile.id}
                  style={{
                    borderBottom:
                      index === profiles.length - 1
                        ? 'none'
                        : '1px solid var(--color-border-subtle)',
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
                    <div
                      style={{
                        display: 'flex',
                        alignItems: 'center',
                        color: 'var(--color-text-muted)',
                      }}
                    >
                      {expandedProfileIds.has(profile.id) ? (
                        <ChevronDown size={20} />
                      ) : (
                        <ChevronRight size={20} />
                      )}
                    </div>

                    <div style={{ flex: 1, display: 'flex', alignItems: 'center', gap: '12px' }}>
                      <input
                        type="text"
                        className="settings-input-minimal"
                        value={profile.name}
                        onClick={(event) => event.stopPropagation()}
                        onChange={(event) =>
                          handleUpdateProfile(profile.id, { name: event.target.value })
                        }
                        style={{
                          fontWeight: 600,
                          fontSize: '1rem',
                          width: 'auto',
                          minWidth: '150px',
                        }}
                      />
                      <button
                        type="button"
                        className="dict-scope-pill"
                        onClick={(e) => {
                          e.stopPropagation();
                          openScopeModal(profile);
                        }}
                        style={{
                          height: '24px',
                          padding: '0 8px',
                          fontSize: '0.75rem',
                          cursor: 'pointer',
                          border:
                            profile.scope === 'project' &&
                            (!profile.projectIds || profile.projectIds.length === 0)
                              ? '1px dashed var(--color-warning)'
                              : undefined,
                        }}
                        data-tooltip={t('settings.speaker_edit_scope', {
                          defaultValue: 'Click to edit target scope',
                        })}
                        data-tooltip-pos="top"
                      >
                        <span style={{ display: 'inline-flex', alignItems: 'center', gap: '4px' }}>
                          {profile.scope === 'project' ? (
                            <>
                              <span
                                style={{
                                  width: '6px',
                                  height: '6px',
                                  borderRadius: '50%',
                                  background:
                                    profile.projectIds && profile.projectIds.length > 0
                                      ? 'var(--color-accent-primary)'
                                      : 'var(--color-warning)',
                                }}
                              />
                              {getScopeBadgeText(profile)}
                            </>
                          ) : (
                            <>
                              <Globe size={11} style={{ opacity: 0.7 }} />
                              <span>
                                {t('settings.dict_scope_badge_global', { defaultValue: 'Global' })}
                              </span>
                            </>
                          )}
                        </span>
                      </button>

                      <span
                        style={{
                          fontSize: '0.75rem',
                          color: 'var(--color-text-muted)',
                          background: 'var(--color-bg-secondary)',
                          padding: '2px 8px',
                          borderRadius: 'var(--radius-sm)',
                        }}
                      >
                        {t('settings.speaker_samples_count', {
                          count: profile.samples.length,
                          defaultValue: `${profile.samples.length} samples`,
                        })}
                      </span>
                      <span style={{ fontSize: '0.75rem', color: 'var(--color-text-secondary)' }}>
                        {readinessCopy}
                      </span>
                    </div>
                    <div
                      style={{ display: 'flex', alignItems: 'center', gap: '20px' }}
                      onClick={(event) => event.stopPropagation()}
                    >
                      <Switch
                        checked={profile.enabled}
                        onChange={(checked) =>
                          handleUpdateProfile(profile.id, { enabled: checked })
                        }
                      />

                      <button
                        className="btn btn-icon btn-danger-soft"
                        onClick={() => void handleDeleteProfile(profile)}
                        data-tooltip={t('settings.delete_speaker_profile', {
                          name: profile.name,
                          defaultValue: 'Delete {{name}}',
                        })}
                        data-tooltip-pos="top"
                        aria-label={t('settings.delete_speaker_profile', {
                          name: profile.name,
                          defaultValue: 'Delete {{name}}',
                        })}
                      >
                        <Trash2 size={18} />
                      </button>
                    </div>
                  </div>

                  {expandedProfileIds.has(profile.id) && (
                    <div
                      style={{
                        padding: '0 24px 24px 56px',
                        display: 'flex',
                        flexDirection: 'column',
                        gap: '12px',
                      }}
                    >
                      <div
                        style={{
                          display: 'flex',
                          justifyContent: 'space-between',
                          alignItems: 'center',
                          gap: '12px',
                          flexWrap: 'wrap',
                        }}
                      >
                        <p
                          style={{
                            margin: 0,
                            color: 'var(--color-text-muted)',
                            fontSize: '0.85rem',
                          }}
                        >
                          {t('settings.speaker_profile_samples_hint', {
                            defaultValue:
                              'Import one or more local reference clips. They will be normalized to 16k mono WAV and stored under app-managed data.',
                          })}
                        </p>
                        <div style={{ color: 'var(--color-text-muted)', fontSize: '0.8rem' }}>
                          {t('settings.speaker_profile_readiness_meta', {
                            usable: readiness.usableSampleCount,
                            duration: formatSampleDuration(readiness.usableDurationSeconds),
                            defaultValue: `${readiness.usableSampleCount} usable samples · ${formatSampleDuration(readiness.usableDurationSeconds)}`,
                          })}
                        </div>
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
                          {t('settings.no_speaker_samples', {
                            defaultValue: 'No reference samples imported yet.',
                          })}
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
                              data-tooltip={t('common.delete')}
                              data-tooltip-pos="top"
                            >
                              <Trash2 size={14} />
                            </button>
                          </div>
                        ))
                      )}
                    </div>
                  )}
                </div>
              );
            })
          )}
        </div>
      </SettingsSection>

      {/* Modal for editing scope */}
      {editingScopeProfile && (
        <Modal
          isOpen={true}
          onClose={() => setEditingScopeProfile(null)}
          title={t('settings.speaker_edit_scope_title', {
            name: editingScopeProfile.name,
            defaultValue: `Edit Scope: ${editingScopeProfile.name}`,
          })}
        >
          <div style={{ display: 'flex', flexDirection: 'column', gap: '16px', padding: '16px 0' }}>
            <label style={{ display: 'flex', alignItems: 'center', gap: '8px', cursor: 'pointer' }}>
              <input
                type="radio"
                name="speaker-scope"
                checked={scopeModalType === 'global'}
                onChange={() => setScopeModalType('global')}
              />
              <div>
                <div style={{ fontWeight: 600, fontSize: '0.875rem' }}>
                  {t('settings.dict_scope_badge_global', { defaultValue: 'Global' })}
                </div>
                <div style={{ fontSize: '0.75rem', color: 'var(--color-text-muted)' }}>
                  {t('settings.speaker_scope_global_desc', {
                    defaultValue: 'Active across all projects, inbox, and global live recordings.',
                  })}
                </div>
              </div>
            </label>

            <label
              style={{ display: 'flex', alignItems: 'flex-start', gap: '8px', cursor: 'pointer' }}
            >
              <input
                type="radio"
                name="speaker-scope"
                checked={scopeModalType === 'project'}
                onChange={() => setScopeModalType('project')}
                style={{ marginTop: '3px' }}
              />
              <div style={{ flex: 1 }}>
                <div style={{ fontWeight: 600, fontSize: '0.875rem' }}>
                  {t('settings.speaker_scope_specific_projects', {
                    defaultValue: 'Specific Projects',
                  })}
                </div>
                <div
                  style={{
                    fontSize: '0.75rem',
                    color: 'var(--color-text-muted)',
                    marginBottom: '8px',
                  }}
                >
                  {t('settings.speaker_scope_specific_projects_desc', {
                    defaultValue: 'Active only in selected projects.',
                  })}
                </div>

                {scopeModalType === 'project' && (
                  <div
                    style={{
                      display: 'flex',
                      flexDirection: 'column',
                      gap: '8px',
                      padding: '12px',
                      background: 'var(--color-bg-secondary)',
                      borderRadius: 'var(--radius-md, 8px)',
                      maxHeight: '200px',
                      overflowY: 'auto',
                    }}
                  >
                    {projects.length === 0 ? (
                      <div style={{ fontSize: '0.75rem', color: 'var(--color-text-muted)' }}>
                        {t('settings.no_projects_available', {
                          defaultValue: 'No projects created yet.',
                        })}
                      </div>
                    ) : (
                      projects.map((proj) => (
                        <label
                          key={proj.id}
                          style={{
                            display: 'flex',
                            alignItems: 'center',
                            gap: '8px',
                            fontSize: '0.8125rem',
                            cursor: 'pointer',
                          }}
                        >
                          <input
                            type="checkbox"
                            checked={scopeModalProjectIds.includes(proj.id)}
                            onChange={(e) => {
                              if (e.target.checked) {
                                setScopeModalProjectIds([...scopeModalProjectIds, proj.id]);
                              } else {
                                setScopeModalProjectIds(
                                  scopeModalProjectIds.filter((id) => id !== proj.id)
                                );
                              }
                            }}
                          />
                          <span>{proj.name}</span>
                        </label>
                      ))
                    )}
                  </div>
                )}
              </div>
            </label>

            <div
              style={{ display: 'flex', justifyContent: 'flex-end', gap: '8px', marginTop: '12px' }}
            >
              <button
                type="button"
                className="btn btn-secondary"
                onClick={() => setEditingScopeProfile(null)}
              >
                {t('common.cancel', { defaultValue: 'Cancel' })}
              </button>
              <button type="button" className="btn btn-primary" onClick={handleSaveScopeModal}>
                {t('common.save', { defaultValue: 'Save' })}
              </button>
            </div>
          </div>
        </Modal>
      )}
    </>
  );
}
