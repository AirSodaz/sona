## 2025-05-21 - Accessible Tooltips
**Learning:** This app uses CSS-only tooltips (`data-tooltip`). These are invisible to screen readers.
**Action:** When seeing `data-tooltip`, always ensure there is a corresponding `aria-label` for accessibility.

## 2025-05-21 - Hidden Interactive Elements
**Learning:** Elements like timestamps were hidden (`opacity: 0`) until hover, making them inaccessible to keyboard users.
**Action:** Always ensure interactive elements are visible on `:focus-visible` (e.g. `opacity: 1`) and have a clear visual indicator.
