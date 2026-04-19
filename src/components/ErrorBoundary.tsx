import { Component, ErrorInfo, ReactNode } from 'react';
import i18n from '../i18n';
import { logger } from '../utils/logger';
import { buildErrorDialogOptions } from '../utils/errorUtils';

interface Props {
    children?: ReactNode;
    fallback?: ReactNode;
}

interface State {
    hasError: boolean;
    error: Error | null;
    errorInfo: ErrorInfo | null;
}

export class ErrorBoundary extends Component<Props, State> {
    public state: State = {
        hasError: false,
        error: null,
        errorInfo: null
    };

    public static getDerivedStateFromError(error: Error): State {
        return { hasError: true, error, errorInfo: null };
    }

    public componentDidCatch(error: Error, errorInfo: ErrorInfo) {
        logger.error('Uncaught error:', error, errorInfo);
        void logger.error('[ErrorBoundary] Uncaught error', error, errorInfo);
        this.setState({ error, errorInfo });
    }

    public render() {
        if (this.state.hasError) {
            if (this.props.fallback) {
                return this.props.fallback;
            }

            return (
                <div style={{
                    padding: '24px',
                    color: 'var(--color-text-primary, #171717)',
                    background: 'var(--color-bg-secondary, #f3f3f2)',
                    height: '100%',
                    overflow: 'auto',
                    display: 'flex',
                    flexDirection: 'column',
                    gap: '16px'
                }}>
                    {(() => {
                        const errorContent = buildErrorDialogOptions(i18n.t.bind(i18n), {
                            code: 'runtime.unexpected',
                            messageKey: 'errors.runtime.unexpected',
                            cause: this.state.error,
                            showCause: import.meta.env.DEV,
                        });

                        return (
                            <>
                                <div>
                                    <h2 style={{ margin: 0, color: 'var(--color-error, #e03e3e)' }}>
                                        {errorContent.title}
                                    </h2>
                                    <p style={{ margin: '8px 0 0', lineHeight: 1.6 }}>
                                        {errorContent.message}
                                    </p>
                                </div>

                                {import.meta.env.DEV && (
                                    <details style={{ whiteSpace: 'pre-wrap', fontFamily: 'monospace' }}>
                                        <summary>{i18n.t('common.details', { defaultValue: 'Details' })}</summary>
                                        {errorContent.details}
                                        {errorContent.details && this.state.errorInfo ? '\n\n' : ''}
                                        {this.state.errorInfo?.componentStack}
                                    </details>
                                )}
                            </>
                        );
                    })()}
                </div>
            );
        }

        return this.props.children;
    }
}
