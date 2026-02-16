import React from 'react';
import { useTranslation } from 'react-i18next';
import { openUrl } from '@tauri-apps/plugin-opener';
import packageJson from '../../../package.json';
import { WaveformIcon, GithubIcon, HeartIcon, ExternalLinkIcon } from '../Icons';

/**
 * Redesigned About page with centered card-based layout.
 * Features a hero section with app logo, info cards, and action buttons.
 */
export function SettingsAboutTab(): React.JSX.Element {
    const { t } = useTranslation();

    const handleOpenHomepage = async () => {
        try {
            await openUrl('https://github.com/AirSodaz/sona');
        } catch (error) {
            console.error('Failed to open homepage:', error);
        }
    };

    const handleCheckUpdates = async () => {
        try {
            await openUrl('https://github.com/AirSodaz/sona/releases');
        } catch (error) {
            console.error('Failed to open releases page:', error);
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
                <button
                    className="btn btn-primary"
                    onClick={handleCheckUpdates}
                >
                    {t('settings.about_check_updates')}
                </button>
            </div>

            {/* Footer */}
            <div className="about-footer">
                <span>{t('settings.about_license')}</span>
            </div>
        </div>
    );
}
