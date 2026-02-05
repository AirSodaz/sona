
## 2024-05-23 - Zustand List Optimization
**Learning:** Subscribing to a mapped array (e.g., `state.items.map(i => i.id)`) triggers re-renders on every store update because the array reference changes.
**Action:** Use `useShallow` from `zustand/react/shallow` to prevent re-renders when the content of the array (IDs) remains the same.

## 2024-05-24 - Batch Processing Store Updates
**Learning:** Frequent store updates (e.g. streaming 100+ segments/sec) cause excessive re-renders even with virtualized lists, due to O(N) array copying and component re-evaluation.
**Action:** Buffer streaming updates in the store action and flush them periodically (e.g. every 500ms or 50 items) to throttle state updates without affecting data integrity.
