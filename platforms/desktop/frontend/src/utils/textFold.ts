/** Folds diacritics and compatibility forms so equivalent text compares equally. */
export function foldText(value: string): string {
  return value.normalize('NFKD').replace(/\p{M}/gu, '');
}
