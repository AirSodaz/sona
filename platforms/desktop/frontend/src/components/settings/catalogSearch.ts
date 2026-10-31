import type { ModelCatalogSectionType } from '../../types/modelCatalog';

/**
 * Search aliases that should expand companion catalog groups even when the
 * Recognition haystack has no hits (e.g. query "vad" / "silero").
 */
export const CATALOG_SECTION_ALIASES: Record<string, ModelCatalogSectionType[]> = {
  asr: ['asr'],
  recognition: ['asr'],
  speech: ['asr'],
  punctuation: ['punctuation'],
  punct: ['punctuation'],
  ct: ['punctuation'],
  vad: ['vad'],
  silero: ['vad'],
  'voice activity': ['vad'],
  speaker: ['speaker-segmentation', 'speaker-embedding'],
  diarization: ['speaker-segmentation', 'speaker-embedding'],
  segmentation: ['speaker-segmentation'],
  embedding: ['speaker-embedding'],
  alignment: ['alignment'],
  ctc: ['alignment'],
};

export function resolveCatalogSectionAliases(query: string): ModelCatalogSectionType[] {
  const normalized = query.trim().toLowerCase();
  if (!normalized) {
    return [];
  }

  const matched = new Set<ModelCatalogSectionType>();
  for (const [alias, sections] of Object.entries(CATALOG_SECTION_ALIASES)) {
    // Prefer exact / contains-alias matches; avoid short-prefix false positives ("a" → asr).
    if (
      normalized === alias ||
      normalized.includes(alias) ||
      (normalized.length >= 3 && alias.startsWith(normalized))
    ) {
      for (const section of sections) {
        matched.add(section);
      }
    }
  }
  return Array.from(matched);
}

/** True when the query is essentially a section alias (show all models in matched sections). */
export function isPrimarilySectionAliasQuery(query: string): boolean {
  const normalized = query.trim().toLowerCase();
  if (!normalized) {
    return false;
  }
  return Object.keys(CATALOG_SECTION_ALIASES).some(
    (alias) => normalized === alias || alias === normalized
  );
}
