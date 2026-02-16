import React from 'react';
import { useTranslation } from 'react-i18next';
import { openUrl } from '@tauri-apps/plugin-opener';
import packageJson from '../../../package.json';

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
            className="settings-group"
            role="tabpanel"
            id="settings-panel-about"
            aria-labelledby="settings-tab-about"
            tabIndex={0}
        >
            <div className="settings-item">
                <label className="settings-label">{t('settings.about')}</label>
                <div className="settings-text" style={{ color: 'var(--color-text-secondary)', lineHeight: '1.6', maxWidth: '500px' }}>
                    {t('settings.about_desc')}
                </div>
            </div>

            <div className="settings-item">
                <label className="settings-label">{t('settings.about_homepage')}</label>
                <div>
                    <button
                        className="btn btn-secondary"
                        onClick={handleOpenHomepage}
                        style={{ display: 'flex', alignItems: 'center', gap: '8px' }}
                    >
                        github.com/AirSodaz/sona
                    </button>
                </div>
            </div>

            <div className="settings-item">
                <label className="settings-label">{t('settings.about_version')}</label>
                <div style={{ fontSize: '1.1em', fontWeight: 500 }}>
                    v{packageJson.version}
                </div>
            </div>

            <div className="settings-item" style={{ marginTop: '24px' }}>
                <button
                    className="btn btn-primary"
                    onClick={handleCheckUpdates}
                >
                    {t('settings.about_check_updates')}
                </button>
            </div>
        </div>
    );
}
