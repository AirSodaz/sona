
## 2024-05-23 - Zustand List Optimization
**Learning:** Subscribing to a mapped array (e.g., `state.items.map(i => i.id)`) triggers re-renders on every store update because the array reference changes.
**Action:** Use `useShallow` from `zustand/react/shallow` to prevent re-renders when the content of the array (IDs) remains the same.
