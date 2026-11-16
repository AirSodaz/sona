import type { DefaultNamespace, Namespace } from 'i18next';

/**
 * i18next 27 declares `TFunction` as a single rest-parameter signature over a union of argument
 * tuples (some of which require the second argument). That makes it unassignable to the minimal
 * translate contract used throughout the app (`(key: string, options?) => string`), which helper
 * props and test doubles rely on.
 *
 * This augmentation adds that minimal call signature to `TFunction` so a real `t` satisfies the
 * contract. The upstream overloads stay first in declaration order, so strict interpolation checks
 * and key typing are unchanged for direct `t(...)` calls.
 */
declare module 'i18next' {
  interface TFunction<Ns extends Namespace = DefaultNamespace, KPrefix = undefined> {
    // biome-ignore lint/style/useShorthandFunctionType: interface merging needs a call signature; a type alias cannot augment i18next's TFunction.
    (key: string, options?: Record<string, unknown>): string;
  }
}
