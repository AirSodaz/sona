import React from 'react';
import { VISIBLE_LANGUAGE_TAGS } from '../utils/languages';

interface LanguageBadgesProps {
  languages: string[] | undefined;
}

/**
 * Compact language-tag cluster for model cards: shows the first few ISO codes
 * plus a `+N` overflow chip whose tooltip carries the full list, so catalogs
 * with ~100-language models stay on one line.
 */
export const LanguageBadges = React.memo(function LanguageBadges({
  languages,
}: LanguageBadgesProps): React.JSX.Element | null {
  if (!languages || languages.length === 0) {
    return null;
  }

  const visible = languages.slice(0, VISIBLE_LANGUAGE_TAGS);
  const overflow = languages.length - visible.length;

  return (
    <span
      className="model-tags"
      style={{ marginTop: '0' }}
      title={languages.join(', ')}
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
