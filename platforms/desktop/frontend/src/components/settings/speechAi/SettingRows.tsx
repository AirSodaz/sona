import type { ReactNode } from 'react';
import { Dropdown, type DropdownOption } from '../../Dropdown';
import { Switch } from '../../Switch';

export function SectionLabel({ children }: { children: ReactNode }): React.JSX.Element {
  return <div className="speech-ai-section-label">{children}</div>;
}

export function DrillRow({
  id,
  title,
  hint,
  value,
  tone,
  indent,
  onClick,
}: {
  id?: string;
  title: string;
  hint?: string;
  value: string;
  tone?: 'ok' | 'warn' | 'err' | 'busy';
  indent?: boolean;
  onClick: () => void;
}): React.JSX.Element {
  return (
    <button
      type="button"
      id={id}
      className={`settings-item-container layout-horizontal speech-ai-drill${indent ? ' is-child' : ''}`}
      onClick={onClick}
      aria-label={`${title}, ${value}`}
    >
      <span className="settings-item-info">
        <span className="settings-item-title">{title}</span>
        {hint ? <span className="settings-item-hint">{hint}</span> : null}
      </span>
      <span className={`speech-ai-drill-value${tone ? ` is-${tone}` : ''}`}>
        {tone ? <span className="speech-ai-dot" aria-hidden="true" /> : null}
        <span>{value}</span>
        <span aria-hidden="true">›</span>
      </span>
    </button>
  );
}

export function SelectRow({
  id,
  title,
  hint,
  value,
  options,
  onChange,
  disabled,
  indent,
  controlId,
}: {
  id?: string;
  title: string;
  hint?: string;
  value: string;
  options: DropdownOption[];
  onChange: (value: string) => void;
  disabled?: boolean;
  indent?: boolean;
  controlId?: string;
}): React.JSX.Element {
  return (
    <div
      id={id}
      className={`settings-item-container layout-horizontal${indent ? ' is-child' : ''}`}
    >
      <div className="settings-item-info">
        <div className="settings-item-title">{title}</div>
        {hint ? <div className="settings-item-hint">{hint}</div> : null}
      </div>
      <div className="settings-item-action speech-ai-control">
        <Dropdown
          id={controlId}
          aria-label={title}
          value={value}
          options={options}
          onChange={onChange}
          disabled={disabled}
          style={{ width: 190 }}
        />
      </div>
    </div>
  );
}

export function SwitchRow({
  id,
  title,
  hint,
  checked,
  onChange,
  indent,
  hintTone,
}: {
  id?: string;
  title: string;
  hint?: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
  indent?: boolean;
  hintTone?: 'err' | 'warn';
}): React.JSX.Element {
  return (
    <div
      id={id}
      className={`settings-item-container layout-horizontal${indent ? ' is-child' : ''}`}
    >
      <div className="settings-item-info">
        <div className="settings-item-title">{title}</div>
        {hint ? (
          <div className={`settings-item-hint${hintTone ? ` is-${hintTone}` : ''}`}>{hint}</div>
        ) : null}
      </div>
      <div className="settings-item-action">
        <Switch checked={checked} onChange={onChange} aria-label={title} />
      </div>
    </div>
  );
}
