## 2024-05-17 - Added Tooltips to Modal Close Buttons
**Learning:** Found that some icon-only close buttons in modals lacked visual tooltips (`data-tooltip`), making them potentially less intuitive for users who rely on visual hover hints to understand functionality, even if `aria-label` is present for screen readers.
**Action:** Always ensure both `aria-label` and visual `data-tooltip` are added to all icon-only buttons for comprehensive accessibility and UX.
