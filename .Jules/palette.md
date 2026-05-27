## 2026-05-27 - Add aria-expanded to collapsible widgets
**Learning:** When adding expand/collapse toggle buttons for nested menus (e.g. secondary speaker profiles), screen readers require the `aria-expanded` attribute to understand the disclosure state. Omitting this makes the widget inaccessible to keyboard/screen reader users.
**Action:** For all collapsible or disclosure widgets, explicitly include `aria-expanded={boolean}` on the toggle button.
