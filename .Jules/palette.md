## 2026-05-09 - Added accessible aria-label to inputs and icon-only buttons
**Learning:** Found that when inputs use placeholder text or icon-only buttons use custom tooltips/titles without explicit labels, screen readers miss the context. This pattern was present in SettingsShortcutInput, IconPicker, and GlobalDialog.
**Action:** Added explicit aria-label attributes synchronized with the translated strings used in tooltips/placeholders to fix screen reader behavior without breaking visual design.
