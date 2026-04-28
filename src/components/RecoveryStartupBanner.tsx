import React, { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { RefreshCw } from 'lucide-react';
import { useRecoveryStore } from '../stores/recoveryStore';

interface RecoveryStartupBannerProps {
    onOpenRecoveryCenter: () => void;
}

export function RecoveryStartupBanner({
    onOpenRecoveryCenter,
}: RecoveryStartupBannerProps): React.JSX.Element | null {
    const { t } = useTranslation();
    const items = useRecoveryStore((state) => state.items);
    const isLoaded = useRecoveryStore((state) => state.isLoaded);

    const summary = useMemo(() => {
        const batchCount = items.filter((item) => item.source === 'batch_import').length;
        const automationCount = items.filter((item) => item.source === 'automation').length;
        return {
            batchCount,
            automationCount,
            total: items.length,
        };
    }, [items]);

    if (!isLoaded || summary.total === 0) {
        return null;
    }

    return (
        <div className="recovery-startup-banner" role="status" aria-live="polite">
            <div className="recovery-startup-copy">
                <strong>{t('recovery.banner.title')}</strong>
                <span>
                    {t('recovery.banner.body', {
                        count: summary.total,
                        batchCount: summary.batchCount,
                        automationCount: summary.automationCount,
                    })}
                </span>
            </div>
            <div className="recovery-startup-actions">
                <button
                    type="button"
                    className="btn btn-secondary"
                    onClick={onOpenRecoveryCenter}
                >
                    <RefreshCw size={14} />
                    {t('recovery.actions.open_center')}
                </button>
            </div>
        </div>
    );
}
