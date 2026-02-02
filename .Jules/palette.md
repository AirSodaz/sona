## 2025-05-21 - Accessible Tooltips
**Learning:** This app uses CSS-only tooltips (`data-tooltip`). These are invisible to screen readers.
**Action:** When seeing `data-tooltip`, always ensure there is a corresponding `aria-label` for accessibility.

## 2025-05-21 - Hidden Interactive Elements
**Learning:** Elements like timestamps were hidden (`opacity: 0`) until hover, making them inaccessible to keyboard users.
**Action:** Always ensure interactive elements are visible on `:focus-visible` (e.g. `opacity: 1`) and have a clear visual indicator.

## 2026-01-26 - Keyboard Accessible Drop Zones
**Learning:** Large drop zones implemented as `div`s often lack keyboard accessibility.
**Action:** When making a `div` clickable, always add `role="button"`, `tabIndex={0}`, `onKeyDown` (Enter/Space), and an `aria-label`.

## 2025-05-21 - Form Label Association
**Learning:** Custom styled inputs often lack programmatic label association (htmlFor/id), relying only on visual proximity.
**Action:** Always verify `htmlFor` matches `id` even on custom-styled form controls.

## 2025-05-21 - Dynamic Aria Labels
**Learning:** Lists of similar items (like models) need unique accessible names (e.g., "Delete [Model Name]" vs just "Delete").
**Action:** Use dynamic values in `aria-label` for repeated actions in lists.

## 2025-10-26 - Custom Progress Bars
**Learning:** Custom progress indicators built with `div`s are invisible to screen readers without explicit semantics.
**Action:** Always add `role="progressbar"`, `aria-valuenow`, `aria-valuemin`, and `aria-valuemax` to the container, and optionally `aria-live="polite"` to the text wrapper.

## 2025-10-27 - Custom Dropdown Menus
**Learning:** Custom dropdowns built with `div`s and `button`s (like Export menu) completely lack semantics for screen readers.
**Action:** Implement the ARIA Menu pattern: `aria-haspopup`/`aria-expanded` on trigger, `role="menu"` on container, and `role="menuitem"` on items.

## 2025-10-27 - Accessible Modals
**Learning:** Custom modals built with `div`s often lack the `dialog` role, making them confusing for screen reader users who don't know they are in a modal.
**Action:** Always add `role="dialog"`, `aria-modal="true"`, and `aria-labelledby` (pointing to the modal title) to the modal container.

## 2026-01-29 - Container Visibility on Focus
**Learning:** When a container is hidden by default (opacity 0) and shown on hover, child focusable elements remain invisible when tabbed into.
**Action:** Use `.container:focus-within { opacity: 1; }` to reveal the container when any child receives focus.

## 2026-01-30 - Keyboard Accessible Tooltips
**Learning:** CSS-only tooltips using `:hover` are invisible to keyboard users who navigate via Tab.
**Action:** Always include `:focus-visible` alongside `:hover` selectors for tooltip visibility (e.g., `[data-tooltip]:hover::after, [data-tooltip]:focus-visible::after`).

## 2026-01-27 - Inline Styles vs. Global CSS
**Learning:** This repo does not use Tailwind or utility classes; it relies on a large `src/styles/index.css` with semantic classes and CSS variables.
**Action:** Do not introduce new utility classes; instead, define semantic classes in `src/styles/index.css` to replace inline styles, leveraging existing variables (e.g., `--color-border`).

## 2026-05-22 - Dynamic Status Updates
**Learning:** Text that updates to reflect system state (like "Recording Paused") is often missed by screen readers if not focused.
**Action:** Add `aria-live="polite"` to containers displaying status messages so updates are announced automatically.

## 2026-05-22 - Accessible Canvas Visualization
**Learning:** Canvas elements used for visual feedback (like audio waves) are invisible to screen readers.
**Action:** Add `role="img"` and a descriptive `aria-label` to `<canvas>` elements to indicate their purpose.

## 2026-05-23 - Nested Interactive Elements
**Learning:** Nesting a `<button>` inside a container with `role="button"` (like a drop zone) creates invalid semantics and confusing screen reader output.
**Action:** Use a `div` with button styling and `aria-hidden="true"` for the inner element, ensuring the parent container handles the interaction.
