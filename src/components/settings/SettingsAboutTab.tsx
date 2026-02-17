import React from 'react';
import { useTranslation } from 'react-i18next';
import { openUrl } from '@tauri-apps/plugin-opener';
import packageJson from '../../../package.json';
import { WaveformIcon, GithubIcon, HeartIcon, ExternalLinkIcon, ProcessingIcon, CheckIcon, ErrorIcon, DownloadIcon } from '../Icons';
import { useAppUpdater } from '../../hooks/useAppUpdater';

/**
 * Redesigned About page with centered card-based layout.
 * Features a hero section with app logo, info cards, and action buttons.
 */
export function SettingsAboutTab(): React.JSX.Element {
    const { t } = useTranslation();
    const { status, error, updateInfo, checkUpdate, installUpdate, progress } = useAppUpdater();

    React.useEffect(() => {
        const handleTrigger = () => {
            checkUpdate();
        };

        window.addEventListener('trigger-update-check', handleTrigger);
        return () => window.removeEventListener('trigger-update-check', handleTrigger);
    }, [checkUpdate]);

    const handleOpenHomepage = async () => {
        try {
            await openUrl('https://github.com/AirSodaz/sona');
        } catch (error) {
            console.error('Failed to open homepage:', error);
        }
    };

    const renderUpdateContent = () => {
        switch (status) {
            case 'idle':
                return (
                    <button
                        className="btn btn-primary"
                        onClick={checkUpdate}
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
                            onClick={() => window.location.reload()} // Fallback, usually app restarts itself
                        >
                            {t('settings.update_btn_relaunch')}
                        </button>
                    </div>
                );
            case 'error':
                return (
                    <div className="update-status error">
                        <ErrorIcon className="w-5 h-5 text-red-500" style={{ color: 'var(--color-error)' }} />
                        <span className="error-text">{t('settings.update_error', { error })}</span>
                        <button
                            className="btn btn-secondary btn-sm"
                            onClick={checkUpdate}
                        >
                            {t('common.retry', { defaultValue: 'Retry' })}
                        </button>
                    </div>
                );
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
            tabIndex={0}
        >
            {/* Hero Section */}
            <div className="about-header">
                <div className="about-logo">
                    <WaveformIcon />
                </div>
                <div className="about-title">
                    <h2>Sona</h2>
                    <span className="about-version-badge">v{packageJson.version}</span>
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
        </div>
    );
}
