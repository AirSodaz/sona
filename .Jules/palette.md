## 2025-05-23 - Search Input Pattern
**Learning:** The project lacks a standardized "SearchInput" component with clear functionality. I implemented a one-off solution in `HistoryView.tsx` using inline styles to match existing code, but noticed `SearchUI.tsx` has similar needs but different implementation.
**Action:** Future enhancements should consider extracting a shared `SearchInput` component that includes the clear button and Escape key handling to ensure consistency across all search interfaces.
