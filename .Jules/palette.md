## 2024-05-24 - [SettingsShortcutInput Icon Accessibility]
**Learning:** Found a specific pattern in the application where custom icon-only components like `FilePenLine` from `lucide-react` are wrapped in buttons without textual context. While `data-tooltip` provides visual context, it does not suffice for screen readers, leading to silent stops.
**Action:** Always ensure that buttons wrapping icon components, especially custom ones like those from `lucide-react` or `./Icons`, have an explicit `aria-label` matching their tooltip translation key.
