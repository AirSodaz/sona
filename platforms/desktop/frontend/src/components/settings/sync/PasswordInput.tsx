import React from 'react';
import { Eye, EyeOff } from 'lucide-react';
import { useTranslation } from 'react-i18next';
export interface PasswordInputProps {
  id?: string;
  value: string;
  onChange: (e: React.ChangeEvent<HTMLInputElement>) => void;
  placeholder?: string;
  disabled?: boolean;
  autoComplete?: string;
  ariaLabel?: string;
  style?: React.CSSProperties;
  className?: string;
}

export function PasswordInput({
  id,
  value,
  onChange,
  placeholder = '••••••••',
  disabled,
  autoComplete,
  ariaLabel,
  style,
  className = '',
}: PasswordInputProps): React.JSX.Element {
  const { t } = useTranslation();
  const [show, setShow] = React.useState(false);
  return (
    <div className={`sync-password-wrapper ${className}`} style={style}>
      <input
        id={id}
        className="settings-input"
        type={show ? 'text' : 'password'}
        value={value}
        onChange={onChange}
        placeholder={placeholder}
        disabled={disabled}
        autoComplete={autoComplete}
        aria-label={ariaLabel}
      />
      <button
        type="button"
        className="sync-password-toggle"
        onClick={() => setShow(!show)}
        disabled={disabled}
        aria-label={show ? t('settings.sync.hide_password', { defaultValue: 'Hide password' }) : t('settings.sync.show_password', { defaultValue: 'Show password' })}
        tabIndex={-1}
      >
        {show ? <EyeOff size={15} /> : <Eye size={15} />}
      </button>
    </div>
  );
}

export default PasswordInput;
