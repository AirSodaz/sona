import type React from 'react';
import { useTranslation } from 'react-i18next';
import { formatLanguagesTooltip, VISIBLE_LANGUAGE_TAGS } from '../utils/languages';

interface LanguageBadgesProps {
  languages: string[] | undefined;
}

/**
 * Compact language-tag cluster for model cards: shows the first few ISO codes
 * plus a `+N` overflow chip, with a styled custom tooltip showing the full or
 * summarized localized languages list.
 */
export function LanguageBadges({ languages }: LanguageBadgesProps): React.JSX.Element | null {
  const { t, i18n } = useTranslation();

  if (!languages || languages.length === 0) {
    return null;
  }

  const tooltipText = formatLanguagesTooltip(languages, i18n?.language ?? 'zh', t);

  const visible = languages.slice(0, VISIBLE_LANGUAGE_TAGS);
  const overflow = languages.length - visible.length;

  return (
    <span
      className="model-tags language-badges"
      style={{ marginTop: '0' }}
      data-tooltip={tooltipText}
      data-tooltip-pos="top"
      data-tooltip-multiline
      tabIndex={0}
      role="note"
      aria-label={tooltipText}
    >
      {visible.map((code) => (
        <span key={code} className="model-tag">
          {code.toUpperCase()}
        </span>
      ))}
      {overflow > 0 && <span className="model-tag model-tag-overflow">+{overflow}</span>}
    </span>
  );
}
