import { ReactNode, useState } from 'react';
import { ChevronRight } from 'lucide-react';
import './SettingsShared.css';

/**
 * Main container for a settings tab.
 */
export function SettingsTabContainer({ children, id, ariaLabelledby }: { children: ReactNode; id?: string; ariaLabelledby?: string }) {
    return (
        <div
            className="settings-tab-container"
            role="tabpanel"
            id={id}
            aria-labelledby={ariaLabelledby}
            tabIndex={0}
        >
            {children}
        </div>
    );
}

export interface SettingsPageHeaderProps {
    title: string | ReactNode;
    description?: string | ReactNode;
    icon?: ReactNode;
}

export function SettingsPageHeader({ title, description, icon }: SettingsPageHeaderProps) {
    return (
        <div className="settings-page-header">
            <h2 className="settings-page-title">
                {icon && <span className="settings-page-icon">{icon}</span>}
                {title}
            </h2>
            {description && (
                <p className="settings-page-description">
                    {description}
                </p>
            )}
            <div className="settings-divider" />
        </div>
    );
}

/**
 * A grouped section of settings with an optional title, icon, and description.
 */
interface SettingsSectionProps {
    title?: string;
    description?: string;
    icon?: ReactNode;
    children: ReactNode;
}

export function SettingsSection({ title, description, icon, children }: SettingsSectionProps) {
    return (
        <section className="settings-section">
            {(title || description) && (
                <div className="settings-section-header">
                    {title && (
                        <div className="settings-section-title-wrapper">
                            {icon && <span className="settings-section-icon">{icon}</span>}
                            <span>{title}</span>
                        </div>
                    )}
                    {description && (
                        <div className="settings-section-description">{description}</div>
                    )}
                </div>
            )}
            <div className="settings-section-content">
                {children}
            </div>
        </section>
    );
}

/**
 * A single setting row or block.
 */
interface SettingsItemProps {
    title: string | ReactNode;
    hint?: string | ReactNode;
    layout?: 'horizontal' | 'vertical';
    children: ReactNode;
    indent?: boolean;
    style?: React.CSSProperties;
}

export function SettingsItem({
    title,
    hint,
    layout = 'horizontal',
    children,
    indent,
    style,
}: SettingsItemProps) {
    return (
        <div
            className={`settings-item-container layout-${layout} ${indent ? 'indented' : ''}`}
            style={{
                ...style,
                ...(indent ? { paddingLeft: '56px' } : {}),
            }}
        >
            <div className="settings-item-info">
                <div className="settings-item-title">{title}</div>
                {hint && <div className="settings-item-hint">{hint}</div>}
            </div>
            <div className="settings-item-action">
                {children}
            </div>
        </div>
    );
}

/**
 * A collapsible accordion item for advanced settings.
 */
interface SettingsAccordionProps {
    title: string | ReactNode;
    status?: ReactNode;
    defaultOpen?: boolean;
    isOpen?: boolean;
    onToggle?: () => void;
    children: ReactNode;
}

export function SettingsAccordion({ title, status, defaultOpen = false, isOpen, onToggle, children }: SettingsAccordionProps) {
    const [localOpen, setLocalOpen] = useState(defaultOpen);
    
    const isExpanded = isOpen !== undefined ? isOpen : localOpen;
    
    const handleToggle = () => {
        if (onToggle) {
            onToggle();
        } else {
            setLocalOpen(!isExpanded);
        }
    };

    return (
        <div className="accordion-wrapper">
            <button type="button" className="accordion-header-btn" onClick={handleToggle} aria-expanded={isExpanded}>
                <div className="accordion-header-left">
                    <ChevronRight size={18} className={`accordion-chevron ${isExpanded ? 'open' : ''}`} />
                    <span>{title}</span>
                </div>
                {status && <div className="accordion-header-status">{status}</div>}
            </button>
            {isExpanded && (
                <div className="accordion-content-panel">
                    {children}
                </div>
            )}
        </div>
    );
}
