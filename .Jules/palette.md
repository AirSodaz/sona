## 2026-03-10 - Consistent Tooltip UX Pattern
**Learning:** Using native browser `title` attributes on icon buttons leads to inconsistent, unstyled, and slow-to-appear tooltips. The app uses a custom CSS-based `data-tooltip` attribute system (found in `src/styles/index.css`) which provides immediate, visually consistent hover text.
**Action:** Always prefer using `data-tooltip` and `data-tooltip-pos="top|bottom|left|right"` instead of native `title` attributes on UI elements like icon buttons to maintain a cohesive and responsive user experience.
