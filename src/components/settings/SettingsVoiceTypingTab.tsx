import React from 'react';
import { Check, Keyboard, Loader2, Type, Waves, X } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useVoiceTypingReadiness } from '../../hooks/useVoiceTypingReadiness';
import { useSetConfig, useVoiceTypingConfig } from '../../stores/configStore';
import { VoiceTypingRuntimeErrorSource } from '../../stores/voiceTypingRuntimeStore';
import { Dropdown } from '../Dropdown';
import { Switch } from '../Switch';
import { SettingsPageHeader, SettingsSection, SettingsItem, SettingsTabContainer } from './SettingsLayout';
import { useSettingsNavigation } from './SettingsNavigationContext';
import { SettingsShortcutInput } from './SettingsShortcutInput';

type BadgeTone = 'ready' | 'missing' | 'off' | 'pending';

function StatusBadge({
    tone,
    label,
}: {
    tone: BadgeTone;
    label: string;
}): React.JSX.Element {
    const icon =
        tone === 'ready' ? (
            <Check size={12} />
        ) : tone === 'pending' ? (
            <Loader2 size={12} className="animate-spin" />
        ) : (
            <X size={12} />
        );

    return (
        <span className={`status-badge ${tone}`}>
            {icon}
            {label}
        </span>
    );
}

function getFailureSourceLabel(
    t: (key: string, options?: Record<string, unknown>) => string,
    source: VoiceTypingRuntimeErrorSource | null
) {
    switch (source) {
        case 'shortcut_registration':
            return t('settings.voice_typing_failure_source_shortcut', {
                defaultValue: 'Shortcut registration',
            });
        case 'warmup':
            return t('settings.voice_typing_failure_source_warmup', {
                defaultValue: 'Background warm-up',
            });
        case 'microphone':
            return t('settings.voice_typing_failure_source_microphone', {
                defaultValue: 'Input device',
            });
        case 'session':
            return t('settings.voice_typing_failure_source_session', {
                defaultValue: 'Voice Typing session',
            });
        default:
            return null;
    }
}

function VoiceTypingStatusCard(): React.JSX.Element {
    const { t } = useTranslation();
    const readiness = useVoiceTypingReadiness();

    let tone: BadgeTone = 'pending';
    let badgeLabel = t('settings.voice_typing_status_preparing', { defaultValue: 'Preparing' });
    let title = t('settings.voice_typing_status_summary_preparing', {
        defaultValue: 'Voice Typing is getting ready in the background.',
    });
    let description = t('settings.voice_typing_status_detail_preparing', {
        defaultValue: 'Shortcut registration and warm-up will complete automatically when the required dependencies are available.',
    });

    switch (readiness.state) {
        case 'off':
            tone = 'off';
            badgeLabel = t('settings.voice_typing_status_off', { defaultValue: 'Off' });
            title = t('settings.voice_typing_status_summary_off', {
                defaultValue: 'Voice Typing is currently turned off.',
            });
            description = t('settings.voice_typing_status_detail_off', {
                defaultValue: 'Enable Voice Typing to dictate into other apps with your configured shortcut.',
            });
            break;
        case 'needs_shortcut':
            tone = 'missing';
            badgeLabel = t('settings.voice_typing_status_missing_shortcut', {
                defaultValue: 'Missing shortcut',
            });
            title = t('settings.voice_typing_status_summary_missing_shortcut', {
                defaultValue: 'Voice Typing needs a shortcut before it can start.',
            });
            description = t('settings.voice_typing_status_detail_missing_shortcut', {
                defaultValue: 'Set a global shortcut so Sona knows how to start and stop dictation.',
            });
            break;
        case 'needs_live_model':
            tone = 'missing';
            badgeLabel = t('settings.voice_typing_status_missing_model', {
                defaultValue: 'Missing model',
            });
            title = t('settings.voice_typing_status_summary_missing_model', {
                defaultValue: 'Voice Typing needs a Live Record Model.',
            });
            description = t('settings.voice_typing_status_detail_missing_model', {
                defaultValue: 'Voice Typing reuses the live transcription model, so you need to choose one in Model Hub first.',
            });
            break;
        case 'needs_vad':
            tone = 'missing';
            badgeLabel = t('settings.voice_typing_status_missing_vad', {
                defaultValue: 'Missing VAD',
            });
            title = t('settings.voice_typing_status_summary_missing_vad', {
                defaultValue: 'The selected Live Record Model also needs a VAD model.',
            });
            description = t('settings.voice_typing_status_detail_missing_vad', {
                defaultValue: 'Install or select the required VAD model in Model Hub before using Voice Typing.',
            });
            break;
        case 'failed':
            tone = 'missing';
            badgeLabel = t('settings.voice_typing_status_failed', { defaultValue: 'Failed' });
            title = t('settings.voice_typing_status_summary_failed', {
                defaultValue: 'Voice Typing hit a runtime problem.',
            });
            description = readiness.lastErrorMessage
                ? t('settings.voice_typing_last_error', {
                    defaultValue: 'Last error: {{message}}',
                    message: readiness.lastErrorMessage,
                })
                : t('settings.voice_typing_status_detail_failed', {
                    defaultValue: 'Sona observed a Voice Typing error and is waiting for the next valid configuration or warm-up cycle.',
                });
            break;
        case 'ready':
            tone = 'ready';
            badgeLabel = t('settings.voice_typing_status_ready', { defaultValue: 'Ready' });
            title = t('settings.voice_typing_status_summary_ready', {
                defaultValue: 'Voice Typing is ready to dictate into other apps.',
            });
            description = t('settings.voice_typing_status_detail_ready', {
                defaultValue: 'Shortcut registration and background warm-up are complete.',
            });
            break;
        default:
            break;
    }

    const failureSourceLabel = getFailureSourceLabel(t, readiness.lastErrorSource);

    return (
        <div className="settings-status-card">
            <div className="settings-status-card-header">
                <StatusBadge tone={tone} label={badgeLabel} />
            </div>
            <div className="settings-status-card-title">{title}</div>
            <div className="settings-status-card-description">{description}</div>
            {readiness.state === 'failed' && failureSourceLabel ? (
                <div className="settings-status-card-meta">
                    {t('settings.voice_typing_failure_source_label', {
                        defaultValue: 'Source: {{source}}',
                        source: failureSourceLabel,
                    })}
                </div>
            ) : null}
        </div>
    );
}

function VoiceTypingDependenciesSection(): React.JSX.Element {
    const { t } = useTranslation();
    const readiness = useVoiceTypingReadiness();
    const { navigateToTab } = useSettingsNavigation();

    const modelCta = (
        <button
            type="button"
            className="btn btn-secondary"
            onClick={() => navigateToTab('models')}
        >
            {t('settings.voice_typing_open_model_hub', { defaultValue: 'Open Model Hub' })}
        </button>
    );
    const inputCta = (
        <button
            type="button"
            className="btn btn-secondary"
            onClick={() => navigateToTab('microphone')}
        >
            {t('settings.voice_typing_open_input_device', { defaultValue: 'Open Input Device' })}
        </button>
    );

    const runtimeSourceLabel = getFailureSourceLabel(t, readiness.lastErrorSource);

    let shortcutTone: BadgeTone;
    let shortcutLabel: string;
    let shortcutHint: string;
    if (readiness.shortcutConfigured) {
        shortcutTone = 'ready';
        shortcutLabel = t('settings.voice_typing_status_ready', { defaultValue: 'Ready' });
        shortcutHint = t('settings.voice_typing_dependency_shortcut_ready', { defaultValue: 'A global shortcut is configured for starting Voice Typing.' });
    } else {
        shortcutTone = 'missing';
        shortcutLabel = t('settings.voice_typing_status_missing_shortcut', { defaultValue: 'Missing shortcut' });
        shortcutHint = t('settings.voice_typing_dependency_shortcut_missing', { defaultValue: 'Set a global shortcut before Voice Typing can start listening.' });
    }

    let modelTone: BadgeTone;
    let modelLabel: string;
    let modelHint: string;
    if (readiness.liveModelConfigured) {
        modelTone = 'ready';
        modelLabel = t('settings.voice_typing_status_ready', { defaultValue: 'Ready' });
        modelHint = t('settings.voice_typing_dependency_model_ready', { defaultValue: 'Voice Typing can reuse the selected live transcription model.' });
    } else {
        modelTone = 'missing';
        modelLabel = t('settings.voice_typing_status_missing_model', { defaultValue: 'Missing model' });
        modelHint = t('settings.voice_typing_dependency_model_missing', { defaultValue: 'Choose a live transcription model in Model Hub before using Voice Typing.' });
    }

    let vadTone: BadgeTone;
    let vadLabel: string;
    let vadHint: string;
    if (!readiness.requiresVad) {
        vadTone = 'off';
        vadLabel = t('settings.voice_typing_dependency_vad_not_needed_badge', { defaultValue: 'Not needed' });
        vadHint = t('settings.voice_typing_dependency_vad_not_required', { defaultValue: 'The selected live model does not require a separate VAD model.' });
    } else if (readiness.vadConfigured) {
        vadTone = 'ready';
        vadLabel = t('settings.voice_typing_status_ready', { defaultValue: 'Ready' });
        vadHint = t('settings.voice_typing_dependency_vad_ready', { defaultValue: 'The required VAD model is configured.' });
    } else {
        vadTone = 'missing';
        vadLabel = t('settings.voice_typing_status_missing_vad', { defaultValue: 'Missing VAD' });
        vadHint = t('settings.voice_typing_dependency_vad_missing', { defaultValue: 'The selected live model requires a VAD model in Model Hub.' });
    }

    let inputTone: BadgeTone;
    let inputLabel: string;
    let inputHint: string;
    if (readiness.inputDeviceState === 'failed') {
        inputTone = 'missing';
        inputLabel = t('settings.voice_typing_status_failed', { defaultValue: 'Failed' });
        inputHint = readiness.lastErrorMessage || '';
    } else if (readiness.inputDeviceState === 'off') {
        inputTone = 'off';
        inputLabel = t('settings.voice_typing_status_off', { defaultValue: 'Off' });
        inputHint = t('settings.voice_typing_dependency_input_ready', { defaultValue: 'Voice Typing uses the current Input Device setting. The default device is valid.' });
    } else {
        inputTone = 'ready';
        inputLabel = t('settings.voice_typing_status_ready', { defaultValue: 'Ready' });
        inputHint = t('settings.voice_typing_dependency_input_ready', { defaultValue: 'Voice Typing uses the current Input Device setting. The default device is valid.' });
    }

    let runtimeTone: BadgeTone;
    let runtimeLabel: string;
    let runtimeHint: string;
    if (readiness.runtimeState === 'failed') {
        runtimeTone = 'missing';
        runtimeLabel = t('settings.voice_typing_status_failed', { defaultValue: 'Failed' });
        if (runtimeSourceLabel) {
            runtimeHint = t('settings.voice_typing_dependency_runtime_failed_with_source', {
                defaultValue: '{{source}}: {{message}}',
                source: runtimeSourceLabel,
                message: readiness.lastErrorMessage,
            });
        } else {
            runtimeHint = readiness.lastErrorMessage || '';
        }
    } else if (readiness.runtimeState === 'ready') {
        runtimeTone = 'ready';
        runtimeLabel = t('settings.voice_typing_status_ready', { defaultValue: 'Ready' });
        runtimeHint = t('settings.voice_typing_dependency_runtime_ready', { defaultValue: 'Shortcut registration and warm-up have completed.' });
    } else if (readiness.runtimeState === 'off') {
        runtimeTone = 'off';
        runtimeLabel = t('settings.voice_typing_status_off', { defaultValue: 'Off' });
        runtimeHint = t('settings.voice_typing_dependency_runtime_off', { defaultValue: 'Background warm-up starts only when Voice Typing is enabled.' });
    } else {
        runtimeTone = 'pending';
        runtimeLabel = t('settings.voice_typing_status_preparing', { defaultValue: 'Preparing' });
        runtimeHint = t('settings.voice_typing_dependency_runtime_preparing', { defaultValue: 'Sona is waiting for registration and warm-up to settle.' });
    }

    return (
        <SettingsSection
            title={t('settings.voice_typing_dependencies', {
                defaultValue: 'Readiness And Dependencies',
            })}
            description={t('settings.voice_typing_dependencies_description', {
                defaultValue: 'Voice Typing depends on a shortcut, live transcription model, and background warm-up state.',
            })}
            icon={<Waves size={20} />}
        >
            <SettingsItem
                title={t('settings.voice_typing_dependency_shortcut', { defaultValue: 'Shortcut' })}
                hint={shortcutHint}
            >
                <StatusBadge tone={shortcutTone} label={shortcutLabel} />
            </SettingsItem>

            <SettingsItem
                title={t('settings.voice_typing_dependency_model', { defaultValue: 'Live Record Model' })}
                hint={modelHint}
            >
                <div className="settings-status-actions">
                    <StatusBadge tone={modelTone} label={modelLabel} />
                    {!readiness.liveModelConfigured ? modelCta : null}
                </div>
            </SettingsItem>

            <SettingsItem
                title={t('settings.voice_typing_dependency_vad', { defaultValue: 'VAD Model' })}
                hint={vadHint}
            >
                <div className="settings-status-actions">
                    <StatusBadge tone={vadTone} label={vadLabel} />
                    {readiness.requiresVad && !readiness.vadConfigured ? modelCta : null}
                </div>
            </SettingsItem>

            <SettingsItem
                title={t('settings.voice_typing_dependency_input', { defaultValue: 'Input Device' })}
                hint={inputHint}
            >
                <div className="settings-status-actions">
                    <StatusBadge tone={inputTone} label={inputLabel} />
                    {readiness.inputDeviceState === 'failed' ? inputCta : null}
                </div>
            </SettingsItem>

            <SettingsItem
                title={t('settings.voice_typing_dependency_runtime', {
                    defaultValue: 'Background Status',
                })}
                hint={runtimeHint}
            >
                <StatusBadge tone={runtimeTone} label={runtimeLabel} />
            </SettingsItem>
        </SettingsSection>
    );
}

export function SettingsVoiceTypingTab(): React.JSX.Element {
    const { t } = useTranslation();
    const vtConfig = useVoiceTypingConfig();
    const updateConfig = useSetConfig();

    return (
        <SettingsTabContainer
            id="settings-panel-voice_typing"
            ariaLabelledby="settings-tab-voice_typing"
        >
            <SettingsPageHeader
                icon={<Type size={28} />}
                title={t('settings.voice_typing', { defaultValue: 'Voice Typing' })}
                description={t('settings.voice_typing_description', {
                    defaultValue: 'Configure dictation into other applications and see whether Voice Typing is ready to run.',
                })}
            />

            <VoiceTypingStatusCard />

            <SettingsSection
                title={t('settings.voice_typing', { defaultValue: 'Voice Typing' })}
                icon={<Keyboard size={20} />}
            >
                <SettingsItem
                    title={t('settings.enable_voice_typing', {
                        defaultValue: 'Enable Voice Typing',
                    })}
                    hint={t('settings.enable_voice_typing_hint', {
                        defaultValue: 'Type text directly into any application using your voice',
                    })}
                >
                    <Switch
                        checked={vtConfig.voiceTypingEnabled ?? false}
                        onChange={(val) => updateConfig({ voiceTypingEnabled: val })}
                    />
                </SettingsItem>

                <SettingsItem
                    title={t('settings.voice_typing_shortcut', { defaultValue: 'Shortcut' })}
                    hint={t('settings.voice_typing_shortcut_hint', {
                        defaultValue: 'Global shortcut to activate voice typing',
                    })}
                >
                    <SettingsShortcutInput
                        value={vtConfig.voiceTypingShortcut ?? 'Alt+V'}
                        onChange={(val) => updateConfig({ voiceTypingShortcut: val })}
                    />
                </SettingsItem>

                <SettingsItem
                    title={t('settings.voice_typing_mode', { defaultValue: 'Mode' })}
                    hint={t('settings.voice_typing_mode_hint', {
                        defaultValue: 'How the shortcut triggers listening',
                    })}
                >
                    <div style={{ width: '180px' }}>
                        <Dropdown
                            id="vt-mode-select"
                            value={vtConfig.voiceTypingMode || 'hold'}
                            onChange={(val) =>
                                updateConfig({ voiceTypingMode: val as 'hold' | 'toggle' })
                            }
                            options={[
                                {
                                    value: 'hold',
                                    label: t('settings.voice_typing_mode_hold', {
                                        defaultValue: 'Push to Talk (Hold)',
                                    }),
                                },
                                {
                                    value: 'toggle',
                                    label: t('settings.voice_typing_mode_toggle', {
                                        defaultValue: 'Toggle (Press once)',
                                    }),
                                },
                            ]}
                        />
                    </div>
                </SettingsItem>
            </SettingsSection>

            <VoiceTypingDependenciesSection />
        </SettingsTabContainer>
    );
}
