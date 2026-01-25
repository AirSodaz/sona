## 2024-05-22 - [React Virtuoso & Callback Stability]
**Learning:** `react-virtuoso`'s `itemContent` prop is called frequently. If the component passed to it is not memoized, and if the callbacks passed to that component are not stable (e.g. depend on the whole list), every list update (even appending) triggers a re-render of ALL visible items.
**Action:** Always wrap list items in `React.memo` and use `useRef` to access the latest list state in callbacks to keep them stable without adding the list to the dependency array.
