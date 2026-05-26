import React from 'react';

export interface FormFieldProps {
  id?: string;
  label?: React.ReactNode;
  description?: React.ReactNode;
  error?: string;
  children: React.ReactNode;
  required?: boolean;
  layout?: 'vertical' | 'horizontal';
  className?: string;
}

export function FormField({
  id,
  label,
  description,
  error,
  children,
  required = false,
  layout = 'vertical',
  className = '',
}: FormFieldProps): React.JSX.Element {
  const isHorizontal = layout === 'horizontal';

  return (
    <div className={`shared-form-field layout-${layout} ${className}`}>
      {/* Label and description container */}
      {(label || description) && (
        <div className="shared-form-field-info">
          {label && (
            <label htmlFor={id} className="shared-form-field-label">
              {label}
              {required && <span className="shared-form-field-required">*</span>}
            </label>
          )}
          {description && (
            <span className="shared-form-field-desc">
              {description}
            </span>
          )}
        </div>
      )}

      {/* Form Input / Component slot */}
      <div className="shared-form-field-control" style={isHorizontal ? { flexShrink: 0 } : undefined}>
        {children}
      </div>

      {/* Validation Error Message */}
      {error && (
        <div className="shared-form-field-error">
          {error}
        </div>
      )}
    </div>
  );
}
