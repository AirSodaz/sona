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
  const labelId = id && label ? `${id}-label` : undefined;
  const shouldLabelChild = (
    labelId
    && React.isValidElement(children)
    && children.type !== React.Fragment
    && !(children.props as Record<string, unknown>)['aria-label']
    && !(children.props as Record<string, unknown>)['aria-labelledby']
  );
  const control = shouldLabelChild
    ? React.cloneElement(
      children as React.ReactElement<Record<string, unknown>>,
      {
        'aria-labelledby': labelId,
      },
    )
    : children;

  return (
    <div className={`shared-form-field layout-${layout} ${className}`}>
      {/* Label and description container */}
      {(label || description) && (
        <div className="shared-form-field-info">
          {label && (
            <label id={labelId} htmlFor={id} className="shared-form-field-label">
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
        {control}
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
