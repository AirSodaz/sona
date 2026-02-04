## 2025-05-23 - Keyboard Navigation for Dropdown Menus
**Learning:** For custom dropdown menus using `role="menu"`, items should use `tabindex="-1"` to remove them from the natural tab sequence, allowing focus to be managed programmatically via Arrow keys. This prevents Tab from navigating through every menu item, which is the expected behavior for application menus. Also, Vitest setup lacks `jest-dom`, so standard matchers like `toBeInTheDocument` are unavailable.
**Action:** When implementing menus, implement `onKeyDown` for Arrows/Escape/Home/End and use `tabindex="-1"` on items. Use `expect(document.activeElement).toBe(el)` for focus assertions.

## 2025-05-24 - Actionable Empty States
**Learning:** Empty states that are purely informational ("No items yet") can be improved by adding direct action buttons to key workflows (e.g., "Start Recording"). This transforms a dead-end UI into a helpful guide, especially for first-time users.
**Action:** When designing empty states, always ask "What should the user do next?" and provide a shortcut button for that action.
