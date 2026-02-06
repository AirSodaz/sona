## 2024-05-23 - [Keyboard Accessibility in Custom Primitives]
**Learning:** Custom UI primitives like `Dropdown` often miss basic keyboard navigation (Arrow keys, Enter, Escape) that native elements provide for free, creating "accessibility black holes" where users get trapped.
**Action:** When auditing a codebase, check widely used shared components (Dropdowns, Modals, Tooltips) first for keyboard support, as fixing them improves the entire app instantly.
