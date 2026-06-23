import React from 'react';
import { useTranslation } from 'react-i18next';
import { openUrl } from '@tauri-apps/plugin-opener';
import packageJson from '../../../package.json';
import { GithubIcon, HeartIcon, ExternalLinkIcon, ProcessingIcon, CheckIcon, DownloadIcon, BookIcon } from '../Icons';
import { useAppUpdater } from '../../hooks/useAppUpdater';
import { useConfigStore } from '../../stores/configStore';
import { logger } from '../../utils/logger';
import { openLogFolder } from '../../services/tauri/app';

/**
 * Redesigned About page with centered card-based layout.
 * Features a hero section with app logo, info cards, and action buttons.
 */
export function SettingsAboutTab(): React.JSX.Element {
    const { t } = useTranslation();
    const { status, updateInfo, checkUpdate, installUpdate, progress, relaunchToUpdate, crossChannelDownloadUrl } = useAppUpdater();
    const channel = useConfigStore((s) => s.config.channel ?? 'stable');
    const setConfig = useConfigStore((s) => s.setConfig);
    const [showChannelConfirm, setShowChannelConfirm] = React.useState(false);
    const [pendingChannel, setPendingChannel] = React.useState<'stable' | 'nightly' | null>(null);

    const applyChannelChange = (newChannel: 'stable' | 'nightly') => {
        setConfig({ channel: newChannel });
        void checkUpdate({ manual: true, channelSwitch: true });
    };

    const handleChannelChange = (newChannel: 'stable' | 'nightly') => {
        if (newChannel === channel) return;

        if (newChannel === 'nightly') {
            setPendingChannel(newChannel);
            setShowChannelConfirm(true);
        } else {
            applyChannelChange(newChannel);
        }
    };

    const confirmChannelSwitch = () => {
        if (pendingChannel) {
            applyChannelChange(pendingChannel);
        }
        setShowChannelConfirm(false);
        setPendingChannel(null);
    };

    const cancelChannelSwitch = () => {
        setShowChannelConfirm(false);
        setPendingChannel(null);
    };

    const handleOpenHomepage = async () => {
        try {
            await openUrl('https://github.com/AirSodaz/sona');
        } catch (error) {
            logger.error('Failed to open homepage:', error);
        }
    };

    const handleOpenLogFolder = async () => {
        try {
            await openLogFolder();
        } catch (error) {
            logger.error('Failed to open log folder:', error);
        }
    };

    const renderUpdateContent = () => {
        switch (status) {
            case 'idle':
                return (
                    <button
                        className="btn btn-primary"
                        onClick={() => checkUpdate({ manual: true })}
                    >
                        {t('settings.about_check_updates')}
                    </button>
                );
            case 'checking':
                return (
                    <div className="update-status">
                        <ProcessingIcon className="w-5 h-5 text-primary queue-icon-spin" />
                        <span>{t('settings.update_checking')}</span>
                    </div>
                );
            case 'available':
                if (crossChannelDownloadUrl) {
                    const isStable = channel === 'stable';
                    return (
                        <div className="update-available-container">
                            <div className="update-info">
                                <DownloadIcon className="w-5 h-5 text-primary" />
                                <span>
                                    {t('settings.update_available', { version: updateInfo?.version || (isStable ? t('settings.channel_stable') : t('settings.channel_nightly')) })}
                                </span>
                            </div>
                            <button
                                className="btn btn-primary btn-sm"
                                onClick={async () => {
                                    try {
                                        await openUrl(crossChannelDownloadUrl);
                                    } catch (err) {
                                        logger.error('Failed to open download URL:', err);
                                    }
                                }}
                            >
                                {isStable ? t('settings.channel_download_stable') : t('settings.channel_download_nightly')}
                            </button>
                        </div>
                    );
                }
                return (
                    <div className="update-available-container">
                        <div className="update-info">
                            <DownloadIcon className="w-5 h-5 text-primary" />
                            <span>{t('settings.update_available', { version: updateInfo?.version })}</span>
                        </div>
                        <button
                            className="btn btn-primary btn-sm"
                            onClick={installUpdate}
                        >
                            {t('settings.update_btn_install')}
                        </button>
                    </div>
                );
            case 'uptodate':
                return (
                    <div className="update-status success">
                        <CheckIcon className="w-5 h-5 text-green-500" style={{ color: 'var(--color-success)' }} />
                        <span>{t('settings.update_not_available')}</span>
                    </div>
                );
            case 'downloading':
            case 'installing':
                return (
                    <div className="update-progress-container">
                        <div className="update-progress-header">
                            <span>{status === 'downloading' ? t('settings.update_downloading') : t('settings.update_installing')}</span>
                            <span>{progress}%</span>
                        </div>
                        <div className="progress-bar">
                            <div
                                className="progress-bar-fill"
                                style={{ width: `${progress}%` }}
                            />
                        </div>
                    </div>
                );
            case 'downloaded':
                return (
                    <div className="update-status success">
                        <CheckIcon className="w-5 h-5 text-green-500" style={{ color: 'var(--color-success)' }} />
                        <span>{t('settings.update_relaunch')}</span>
                        <button
                            className="btn btn-primary btn-sm"
                            onClick={relaunchToUpdate}
                        >
                            {t('settings.update_btn_relaunch')}
                        </button>
                    </div>
                );
            case 'error':
            default:
                return null;
        }
    };

    return (
        <div
            className="about-container"
            role="tabpanel"
            id="settings-panel-about"
            aria-labelledby="settings-tab-about"
        >
            {/* Hero Section */}
            <div className="about-header">
                <div className="about-logo">
                    <img src="/sona.svg" alt="Sona Logo" style={{ width: '100%', height: '100%', borderRadius: 'inherit' }} />
                </div>
                <div className="about-title">
                    <h2>Sona</h2>
                    <div className="about-version-row">
                        <span className="about-version-badge">v{packageJson.version}</span>
                        <div className="about-channel-selector">
                            <label htmlFor="channel-select">{t('settings.channel')}:</label>
                            <select
                                id="channel-select"
                                value={channel}
                                onChange={(e) => handleChannelChange(e.target.value as 'stable' | 'nightly')}
                                className="channel-select"
                                aria-label={t('settings.channel')}
                            >
                                <option value="stable">{t('settings.channel_stable')}</option>
                                <option value="nightly">{t('settings.channel_nightly')}</option>
                            </select>
                        </div>
                    </div>
                </div>
                <p className="about-description">
                    {t('settings.about_desc')}
                </p>
            </div>

            {/* Info Cards */}
            <div className="about-cards">
                <button
                    className="about-card about-card-clickable"
                    onClick={handleOpenHomepage}
                    aria-label={t('settings.about_source_code')}
                >
                    <div className="about-card-icon">
                        <GithubIcon />
                    </div>
                    <div className="about-card-content">
                        <div className="about-card-title">{t('settings.about_source_code')}</div>
                        <div className="about-card-subtitle">github.com/AirSodaz/sona</div>
                    </div>
                    <div className="about-card-arrow">
                        <ExternalLinkIcon />
                    </div>
                </button>

                <button
                    className="about-card about-card-clickable"
                    onClick={handleOpenLogFolder}
                    aria-label={t('settings.about_open_logs')}
                >
                    <div className="about-card-icon">
                        <BookIcon />
                    </div>
                    <div className="about-card-content">
                        <div className="about-card-title">{t('settings.about_open_logs')}</div>
                        <div className="about-card-subtitle">{t('settings.about_open_logs_desc')}</div>
                    </div>
                    <div className="about-card-arrow">
                        <ExternalLinkIcon />
                    </div>
                </button>

                <div className="about-card about-card-static">
                    <div className="about-card-icon">
                        <HeartIcon />
                    </div>
                    <div className="about-card-content">
                        <div className="about-card-title">{t('settings.about_built_with')}</div>
                        <div className="about-card-subtitle">{t('settings.about_built_with_desc')}</div>
                    </div>
                </div>
            </div>

            {/* Actions */}
            <div className="about-actions">
                {renderUpdateContent()}
            </div>

            {/* Footer */}
            <div className="about-footer">
                <span>{t('settings.about_license')}</span>
            </div>

            {showChannelConfirm && (
                <div className="shared-modal-overlay" onClick={cancelChannelSwitch}>
                    <div className="shared-modal-shell shared-modal-sm" onClick={(e) => e.stopPropagation()} role="dialog" aria-modal="true" aria-labelledby="channel-confirm-title">
                        <div className="shared-modal-header">
                            <h3 className="shared-modal-title" id="channel-confirm-title">{t('settings.channel_switch_confirm_title')}</h3>
                        </div>
                        <div className="shared-modal-body">
                            <p>{t('settings.channel_switch_confirm_body')}</p>
                        </div>
                        <div className="shared-modal-footer">
                            <button className="btn btn-ghost" onClick={cancelChannelSwitch}>
                                {t('settings.channel_switch_confirm_cancel')}
                            </button>
                            <button className="btn btn-primary" onClick={confirmChannelSwitch}>
                                {t('settings.channel_switch_confirm_confirm')}
                            </button>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}
