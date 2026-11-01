import type { ReactNode } from 'react';
import type { CapabilityStatus } from './speechAiLogic';

export function DrillValue({ value }: { value: string }): React.JSX.Element {
  return (
    <span className="speech-ai-drill-value">
      <span>{value}</span>
      <span aria-hidden="true">›</span>
    </span>
  );
}

export function SettingDrill({
  title,
  hint,
  value,
  onClick,
}: {
  title: string;
  hint?: string;
  value: string;
  onClick: () => void;
}): React.JSX.Element {
  return (
    <button
      type="button"
      className="speech-ai-drill"
      onClick={(event) => {
        event.currentTarget.blur();
        onClick();
      }}
      aria-label={`${title}, ${value}`}
    >
      <span className="settings-item-info">
        <span className="settings-item-title">{title}</span>
        {hint ? <span className="settings-item-hint">{hint}</span> : null}
      </span>
      <DrillValue value={value} />
    </button>
  );
}

function RowCopy({
  icon,
  title,
  status,
  statusLabel,
  detail,
  lines,
  progress,
  progressLabel,
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
  children?: ReactNode;
}): React.JSX.Element {
  return (
    <>
      <span className="speech-ai-icon" aria-hidden="true">
        {icon}
      </span>
      <span className="speech-ai-row-main">
        <span className="speech-ai-title-line">
          <span className="speech-ai-title">{title}</span>
          <span className={`speech-ai-status is-${status}`}>
            <span className="speech-ai-dot" aria-hidden="true" />
            {statusLabel}
          </span>
        </span>
        {detail ? <span className="speech-ai-detail">{detail}</span> : null}
        {lines ? <span className="speech-ai-lines">{lines}</span> : null}
        {progress != null ? (
          <span
            className="speech-ai-progress"
            role="progressbar"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={Math.round(progress)}
            aria-label={progressLabel || statusLabel}
          >
            <span style={{ width: `${Math.max(0, Math.min(100, progress))}%` }} />
          </span>
        ) : null}
        {children}
      </span>
    </>
  );
}

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
  value,
  onActivate,
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
  /** Current value shown, muted, with a chevron. Used when the row itself drills in. */
  value?: string;
  /** Makes the row a button. With `action`, only the text side is the button. */
  onActivate?: () => void;
}): React.JSX.Element {
  const copy = (
    <RowCopy
      icon={icon}
      title={title}
      status={status}
      statusLabel={statusLabel}
      detail={detail}
      lines={lines}
      progress={progress}
      progressLabel={progressLabel}
    >
      {children}
    </RowCopy>
  );
  const accessibleName = value ? `${title}, ${value}` : `${title}, ${statusLabel}`;

  if (onActivate && !action) {
    return (
      <button
        type="button"
        className="speech-ai-row speech-ai-row-button"
        onClick={onActivate}
        aria-label={accessibleName}
      >
        {copy}
        {value ? <DrillValue value={value} /> : <span aria-hidden="true">›</span>}
      </button>
    );
  }

  if (onActivate && action) {
    return (
      <div className="speech-ai-row speech-ai-row-split">
        <button
          type="button"
          className="speech-ai-row-hit"
          onClick={onActivate}
          aria-label={accessibleName}
        >
          {copy}
        </button>
        <div className="speech-ai-actions">{action}</div>
      </div>
    );
  }

  return (
    <section className="speech-ai-row" aria-label={title}>
      {copy}
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
      <span className="speech-ai-link-copy">
        <span className="speech-ai-link-title">{title}</span>
        <span className="speech-ai-link-hint">{hint}</span>
      </span>
      <span aria-hidden="true">›</span>
    </button>
  );
}
