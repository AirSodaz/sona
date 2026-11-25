/** Joins truthy class name fragments into a space-delimited string. */
export function joinClassNames(...parts: Array<string | undefined | false | null>): string {
  return parts.filter(Boolean).join(' ');
}
