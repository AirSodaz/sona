import React, { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { VISIBLE_LANGUAGE_TAGS, formatLanguagesTooltip } from '../utils/languages';

interface LanguageBadgesProps {
  languages: string[] | undefined;
}

/**
 * Compact language-tag cluster for model cards: shows the first few ISO codes
 * plus a `+N` overflow chip, with a styled custom tooltip showing the full or
 * summarized localized languages list.
 */
export const LanguageBadges = React.memo(function LanguageBadges({
  languages,
}: LanguageBadgesProps): React.JSX.Element | null {
  const { t, i18n } = useTranslation();

  const tooltipText = useMemo(() => {
    if (!languages || languages.length === 0) {
      return '';
    }
    return formatLanguagesTooltip(languages, i18n?.language ?? 'zh', t);
  }, [languages, i18n?.language, t]);

  if (!languages || languages.length === 0) {
    return null;
  }

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
        <span key={code} className="model-tag">{code.toUpperCase()}</span>
      ))}
      {overflow > 0 && (
        <span className="model-tag model-tag-overflow">+{overflow}</span>
      )}
    </span>
  );
});
