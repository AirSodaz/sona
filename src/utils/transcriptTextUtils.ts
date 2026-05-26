const NORMALIZE_REGEX = /[^\p{L}\p{N}]/gu;
const PUNCTUATION_ONLY_REGEX = /^[^\p{L}\p{N}]+$/u;
const WHITESPACE_ONLY_REGEX = /^\s+$/;
const LEXER_REGEX = /(<\/?(?:b|i|u)>)|(\s+)|([\p{sc=Han}])|([^<\s\p{sc=Han}]+)|(<)/gui;

export interface TimedTextToken<Timing> {
  text: string;
  timing: Timing;
}

export interface AlignedTextUnit<Timing> {
  text: string;
  timing: Timing;
}

interface FormattedTextUnit {
  text: string;
  normalizedText: string;
}

/** Strips HTML-like formatting tags from transcript text. */
export function stripHtmlTags(text: string): string {
  return text.replace(/<\/?[^>]+(>|$)/g, '');
}

function normalizeAlignmentText(text: string): string {
  return text.toLowerCase().replace(NORMALIZE_REGEX, '');
}

function buildFormattedTextUnits(text: string): FormattedTextUnit[] {
  const words: string[] = [];
  const normalizedWords: string[] = [];
  const activeTags = new Set<string>();

  let match: RegExpExecArray | null;
  LEXER_REGEX.lastIndex = 0;

  while ((match = LEXER_REGEX.exec(text)) !== null) {
    const [full, tag] = match;

    if (tag) {
      const tagName = tag.replace(/[</>]/g, '').toLowerCase();
      if (tag.startsWith('</')) {
        activeTags.delete(tagName);
      } else {
        activeTags.add(tagName);
      }
      continue;
    }

    let wrapped = full;
    const sortedTags = Array.from(activeTags).sort().reverse();
    for (const activeTag of sortedTags) {
      wrapped = `<${activeTag}>${wrapped}</${activeTag}>`;
    }

    const isPunctuation = PUNCTUATION_ONLY_REGEX.test(full) && !WHITESPACE_ONLY_REGEX.test(full);
    const previousWord = words[words.length - 1];
    if (
      words.length > 0
      && isPunctuation
      && !WHITESPACE_ONLY_REGEX.test(stripHtmlTags(previousWord))
    ) {
      words[words.length - 1] += wrapped;
      normalizedWords[normalizedWords.length - 1] = normalizeAlignmentText(stripHtmlTags(words[words.length - 1]));
      continue;
    }

    words.push(wrapped);
    normalizedWords.push(normalizeAlignmentText(stripHtmlTags(wrapped)));
  }

  return words.map((word, index) => ({
    text: word,
    normalizedText: normalizedWords[index],
  }));
}

function buildTokenCharacterIndex(normalizedTokens: string[]): {
  joinedTokens: string;
  charToTokenIndex: number[];
} {
  let joinedTokens = '';
  const charToTokenIndex: number[] = [];

  for (let tokenIndex = 0; tokenIndex < normalizedTokens.length; tokenIndex += 1) {
    const token = normalizedTokens[tokenIndex];
    joinedTokens += token;
    for (let charIndex = 0; charIndex < token.length; charIndex += 1) {
      charToTokenIndex.push(tokenIndex);
    }
  }

  return { joinedTokens, charToTokenIndex };
}

function createFallbackTimingResolver<Timing>(
  tokens: Array<TimedTextToken<Timing>>,
  charToTokenIndex: number[],
): (charPos: number) => Timing | undefined {
  const lastMappedTokenIndex = charToTokenIndex[charToTokenIndex.length - 1];

  return (charPos: number): Timing | undefined => {
    if (tokens.length === 0) {
      return undefined;
    }

    if (charToTokenIndex.length === 0) {
      return tokens[tokens.length - 1].timing;
    }

    if (charPos >= charToTokenIndex.length) {
      return tokens[lastMappedTokenIndex].timing;
    }

    return tokens[charToTokenIndex[charPos]].timing;
  };
}

/** Aligns formatted transcript text units to timing payloads from raw model tokens. */
export function alignTextToTimedTokens<Timing>(
  text: string,
  tokens: Array<TimedTextToken<Timing>>,
): Array<AlignedTextUnit<Timing>> {
  const safeText = typeof text === 'string' ? text : String(text || '');
  if (!safeText || tokens.length === 0) {
    return [];
  }

  const normalizedTokens = tokens.map((token) => (
    typeof token.text === 'string' ? normalizeAlignmentText(token.text) : ''
  ));
  const textUnits = buildFormattedTextUnits(safeText);
  const { joinedTokens, charToTokenIndex } = buildTokenCharacterIndex(normalizedTokens);
  const getFallbackTiming = createFallbackTimingResolver(tokens, charToTokenIndex);
  const result: Array<AlignedTextUnit<Timing>> = [];
  let charPos = 0;
  const maxChars = joinedTokens.length;

  for (let index = 0; index < textUnits.length; index += 1) {
    const unit = textUnits[index];
    const cleanText = stripHtmlTags(unit.text);
    const normalizedText = unit.normalizedText;

    if (!cleanText.trim() || !normalizedText) {
      const timing = getFallbackTiming(charPos);
      if (timing !== undefined) {
        result.push({ text: unit.text, timing });
      }
      continue;
    }

    const searchLimit = Math.max(20, normalizedText.length * 2);
    const searchWindow = joinedTokens.substring(charPos, charPos + searchLimit);
    const localIndex = searchWindow.indexOf(normalizedText);
    let timing = getFallbackTiming(charPos);

    if (localIndex !== -1) {
      const matchPos = charPos + localIndex;
      timing = getFallbackTiming(matchPos);
      charPos = matchPos + normalizedText.length;
    } else {
      let nextNormalizedText = '';
      for (let nextIndex = index + 1; nextIndex < textUnits.length; nextIndex += 1) {
        nextNormalizedText = textUnits[nextIndex].normalizedText;
        if (nextNormalizedText) {
          break;
        }
      }

      if (nextNormalizedText) {
        const nextSearchWindow = joinedTokens.substring(charPos, charPos + searchLimit + nextNormalizedText.length);
        const nextLocalIndex = nextSearchWindow.indexOf(nextNormalizedText);
        charPos = nextLocalIndex !== -1 ? charPos + nextLocalIndex : charPos + 1;
      } else {
        charPos = maxChars;
      }
    }

    if (timing !== undefined) {
      result.push({ text: unit.text, timing });
    }

    if (charPos > maxChars) {
      charPos = maxChars;
    }
  }

  return result;
}
