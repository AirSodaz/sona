import { useEffect, useLayoutEffect } from 'react';

/** SSR-safe layout effect hook that falls back to `useEffect` outside browser environments. */
export const useIsomorphicLayoutEffect =
  typeof window !== 'undefined' ? useLayoutEffect : useEffect;
