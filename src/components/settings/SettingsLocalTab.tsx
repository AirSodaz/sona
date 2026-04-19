import React from 'react';
import { useTranslation } from 'react-i18next';
import { PlaySquare } from 'lucide-react';
import { LocalIcon, RestoreIcon } from '../Icons';
import { Switch } from '../Switch';
import { useTranscriptionConfig, useSetConfig } from '../../stores/configStore';
import { SettingsTabContainer, SettingsSection, SettingsItem, SettingsPageHeader } from './SettingsLayout';
import { useModelManagerContext } from '../../hooks/useModelManager';

export function SettingsLocalTab(): React.JSX.Element {
    const { t } = useTranslation();
    const config = useTranscriptionConfig();
    const updateConfig = useSetConfig();
    const { restoreDefaultModelSettings } = useModelManagerContext();

    const vadBufferSize = config.vadBufferSize || 5;
    const maxConcurrent = config.maxConcurrent || 2;
    const enableITN = config.enableITN ?? true;

    return (
        <SettingsTabContainer id="settings-panel-local" ariaLabelledby="settings-tab-local">
            <SettingsPageHeader 
                icon={<LocalIcon width={28} height={28} />}
                title={t('settings.local_path')} 
                description={t('settings.local_path_description', { defaultValue: 'Configure local transcription and ITN parameters.' })} 
            />
            <SettingsSection
                title={t('settings.transcription_settings')}
                icon={<PlaySquare size={20} />}
                description={t('settings.transcription_settings_hint')}
            >
                <SettingsItem
                    title={t('settings.enable_itn')}
                    hint={t('settings.enable_itn_hint')}
                >
                    <Switch
                        checked={enableITN}
                        onChange={(c) => updateConfig({ enableITN: c })}
                    />
                </SettingsItem>

                <SettingsItem
                    title={t('settings.vad_buffer_size')}
                    hint={t('settings.vad_buffer_hint')}
                >
                    <div style={{ width: '120px' }}>
                        <input
                            id="settings-vad-buffer"
                            type="number"
                            className="settings-input"
                            value={vadBufferSize}
                            onChange={(e) => updateConfig({ vadBufferSize: Number(e.target.value) })}
                            min={0}
                            max={30}
                            step={0.5}
                            style={{ textAlign: 'center' }}
                        />
                    </div>
                </SettingsItem>

                <SettingsItem
                    title={t('settings.max_concurrent_label')}
                    hint={t('settings.max_concurrent_hint')}
                >
                    <div style={{ width: '120px' }}>
                        <input
                            id="settings-max-concurrent"
                            type="number"
                            className="settings-input"
                            value={maxConcurrent}
                            onChange={(e) => {
                                const val = Number(e.target.value);
                                if (val > 0) {
                                    updateConfig({ maxConcurrent: val });
                                }
                            }}
                            min={1}
                            max={4}
                            step={1}
                            style={{ textAlign: 'center' }}
                        />
                    </div>
                </SettingsItem>
            </SettingsSection>

            <div style={{ display: 'flex', justifyContent: 'flex-start', marginTop: '8px' }}>
                <button
                    className="btn btn-restore-defaults"
                    onClick={restoreDefaultModelSettings}
                    aria-label={t('settings.restore_defaults')}
                >
                    <RestoreIcon />
                    {t('settings.restore_defaults')}
                </button>
            </div>
        </SettingsTabContainer>
    );
}

