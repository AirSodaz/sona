import { Search, X } from 'lucide-react';
import type React from 'react';
import { forwardRef, useCallback, useRef } from 'react';
import './SearchInput.css';

export interface SearchInputProps
  extends Omit<React.InputHTMLAttributes<HTMLInputElement>, 'value' | 'onChange' | 'size'> {
  value: string;
  onValueChange: (value: string) => void;
  onChange?: React.ChangeEventHandler<HTMLInputElement>;
  onClear?: () => void;
  clearLabel?: string;
  suffix?: React.ReactNode;
  size?: 'sm' | 'md';
  wrapperClassName?: string;
  inputClassName?: string;
}

export const SearchInput = forwardRef<HTMLInputElement, SearchInputProps>(function SearchInput(
  {
    value,
    onValueChange,
    onClear,
    clearLabel = 'Clear search',
    suffix,
    size = 'md',
    wrapperClassName = '',
    inputClassName = '',
    className,
    disabled,
    readOnly,
    onChange,
    onKeyDown,
    ...inputProps
  },
  forwardedRef
) {
  const localRef = useRef<HTMLInputElement | null>(null);
  const setRef = useCallback(
    (node: HTMLInputElement | null) => {
      localRef.current = node;
      if (typeof forwardedRef === 'function') forwardedRef(node);
      else if (forwardedRef) forwardedRef.current = node;
    },
    [forwardedRef]
  );

  const handleClear = () => {
    if (disabled || readOnly) return;
    if (onClear) onClear();
    else onValueChange('');
    requestAnimationFrame(() => localRef.current?.focus());
  };

  return (
    <div
      className={`sona-search-input sona-search-input--${size} ${wrapperClassName}`.trim()}
      data-testid="search-input"
    >
      <Search className="sona-search-input__icon" aria-hidden="true" />
      <input
        {...inputProps}
        ref={setRef}
        value={value}
        disabled={disabled}
        readOnly={readOnly}
        className={`sona-search-input__field ${inputClassName} ${className ?? ''}`.trim()}
        onChange={(event) => {
          onValueChange(event.target.value);
          onChange?.(event);
        }}
        onKeyDown={onKeyDown}
      />
      {value && !readOnly && (
        <button
          type="button"
          className="sona-search-input__clear"
          onClick={handleClear}
          disabled={disabled}
          aria-label={clearLabel}
        >
          <X size={12} strokeWidth={2.25} />
        </button>
      )}
      {suffix && <div className="sona-search-input__suffix">{suffix}</div>}
    </div>
  );
});

SearchInput.displayName = 'SearchInput';
