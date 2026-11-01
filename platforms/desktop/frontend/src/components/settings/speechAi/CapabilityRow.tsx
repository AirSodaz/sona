import type { ReactNode } from 'react';
import type { CapabilityStatus } from './speechAiLogic';

export function CapabilityRow({
  icon,
  title,
  status,
  statusLabel,
  detail,
  lines,
  progress,
  progressLabel,
  action,
  children,
}: {
  icon: ReactNode;
  title: string;
  status: CapabilityStatus;
  statusLabel: string;
  detail?: ReactNode;
  lines?: ReactNode;
  progress?: number | null;
  progressLabel?: string;
  action?: ReactNode;
  children?: ReactNode;
}): React.JSX.Element {
  return (
    <section className="speech-ai-row" aria-label={title}>
      <span className="speech-ai-icon" aria-hidden="true">
        {icon}
      </span>
      <div className="speech-ai-row-main">
        <div className="speech-ai-title-line">
          <span className="speech-ai-title">{title}</span>
          <span className={`speech-ai-status is-${status}`}>
            <span className="speech-ai-dot" aria-hidden="true" />
            {statusLabel}
          </span>
        </div>
        {detail ? <div className="speech-ai-detail">{detail}</div> : null}
        {lines ? <div className="speech-ai-lines">{lines}</div> : null}
        {progress != null ? (
          <div
            className="speech-ai-progress"
            role="progressbar"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={Math.round(progress)}
            aria-label={progressLabel || statusLabel}
          >
            <span style={{ width: `${Math.max(0, Math.min(100, progress))}%` }} />
          </div>
        ) : null}
        {children}
      </div>
      <div className="speech-ai-actions">{action}</div>
    </section>
  );
}

export function MoreOptionButton({
  id,
  icon,
  title,
  hint,
  onClick,
}: {
  id?: string;
  icon: ReactNode;
  title: string;
  hint: string;
  onClick: () => void;
}): React.JSX.Element {
  return (
    <button type="button" id={id} className="speech-ai-link" onClick={onClick}>
      <span className="speech-ai-icon" aria-hidden="true" style={{ width: 28, height: 28 }}>
        {icon}
      </span>
      <span>
        <span className="speech-ai-link-title">{title}</span>
        <span className="speech-ai-link-hint">{hint}</span>
      </span>
      <span aria-hidden="true">›</span>
    </button>
  );
}
