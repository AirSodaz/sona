## 2024-05-22 - [React Virtuoso & Callback Stability]
**Learning:** `react-virtuoso`'s `itemContent` prop is called frequently. If the component passed to it is not memoized, and if the callbacks passed to that component are not stable (e.g. depend on the whole list), every list update (even appending) triggers a re-render of ALL visible items.
**Action:** Always wrap list items in `React.memo` and use `useRef` to access the latest list state in callbacks to keep them stable without adding the list to the dependency array.

## 2025-02-23 - [React Virtuoso & Context Coupling]
**Learning:** `react-virtuoso` re-renders the entire list if the `context` prop changes, even if items are memoized. Passing frequently changing values (like `activeSegmentId`) in the context forces O(N) re-renders on every update.
**Action:** Decouple frequent updates from `context`. Pass stable callbacks via context, but let individual list items subscribe directly to the store (e.g., using Zustand selectors) for dynamic state that only affects specific items.

## 2026-01-27 - [Virtuoso Context & Ref Timing]
**Learning:** When passing callbacks in `react-virtuoso`'s `context` that rely on current render data (like list length), updating a `ref` in `useEffect` is too late—`itemContent` runs synchronously during render.
**Action:** Update the `ref` synchronously in the render body (before the return statement) to ensure virtualization callbacks see the data from the current render cycle.

## 2025-05-24 - [Canvas Gradient Allocation]
**Learning:** Creating `CanvasGradient` objects in a `requestAnimationFrame` loop (e.g. for audio visualization) generates massive garbage collection pressure (~60k allocations/sec for 1024 bars).
**Action:** Cache gradients based on discrete values (e.g., 0-255 for audio data) and canvas dimensions. Invalidate the cache only when dimensions change.

## 2025-02-27 - [Regex Replace vs Test]
**Learning:** `String.prototype.replace(regex, '')` allocates a new string even if no replacement occurs (depending on the engine, but observed overhead exists). For operations run in tight loops (e.g. 10k+ times), checking `regex.test()` first can save significant time (~50% speedup) if the match frequency is low.
**Action:** When cleaning strings in a loop where the target pattern is rare, guard `replace` with `test` to avoid unnecessary allocations.
