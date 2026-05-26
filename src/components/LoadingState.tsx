import React from 'react';
import { Loader2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';

export interface LoadingStateProps {
  message?: string;
  className?: string;
}

export function LoadingState({
  message,
  className = '',
}: LoadingStateProps): React.JSX.Element {
  const { t } = useTranslation();
  const resolvedMessage = message ?? t('common.loading', { defaultValue: 'Loading...' });

  return (
    <div className={`shared-loading-container ${className}`}>
      <Loader2 className="shared-loading-spinner animate-spin" size={32} />
      <div className="shared-loading-text">{resolvedMessage}</div>
    </div>
  );
}
