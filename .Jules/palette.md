## 2024-05-18 - [Consistent Tooltips for AI Action Buttons]
**Learning:** Native `title` attributes on icon buttons like AI actions create an inconsistent visual experience compared to the app's `data-tooltip` design system. Additionally, they often lack associated `aria-label`s for screen reader support.
**Action:** Use custom `data-tooltip` and `data-tooltip-pos` attributes instead of native `title` for visual tooltips, and always pair them with an `aria-label` for accessibility.
