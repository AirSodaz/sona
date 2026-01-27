## 2024-05-22 - [React Virtuoso & Callback Stability]
**Learning:** `react-virtuoso`'s `itemContent` prop is called frequently. If the component passed to it is not memoized, and if the callbacks passed to that component are not stable (e.g. depend on the whole list), every list update (even appending) triggers a re-render of ALL visible items.
**Action:** Always wrap list items in `React.memo` and use `useRef` to access the latest list state in callbacks to keep them stable without adding the list to the dependency array.

## 2025-02-23 - [React Virtuoso & Context Coupling]
**Learning:** `react-virtuoso` re-renders the entire list if the `context` prop changes, even if items are memoized. Passing frequently changing values (like `activeSegmentId`) in the context forces O(N) re-renders on every update.
**Action:** Decouple frequent updates from `context`. Pass stable callbacks via context, but let individual list items subscribe directly to the store (e.g., using Zustand selectors) for dynamic state that only affects specific items.

## 2026-01-27 - [Virtuoso Context & Ref Timing]
**Learning:** When passing callbacks in `react-virtuoso`'s `context` that rely on current render data (like list length), updating a `ref` in `useEffect` is too late—`itemContent` runs synchronously during render.
**Action:** Update the `ref` synchronously in the render body (before the return statement) to ensure virtualization callbacks see the data from the current render cycle.
