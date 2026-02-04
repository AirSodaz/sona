## 2025-05-23 - Keyboard Navigation for Dropdown Menus
**Learning:** For custom dropdown menus using `role="menu"`, items should use `tabindex="-1"` to remove them from the natural tab sequence, allowing focus to be managed programmatically via Arrow keys. This prevents Tab from navigating through every menu item, which is the expected behavior for application menus. Also, Vitest setup lacks `jest-dom`, so standard matchers like `toBeInTheDocument` are unavailable.
**Action:** When implementing menus, implement `onKeyDown` for Arrows/Escape/Home/End and use `tabindex="-1"` on items. Use `expect(document.activeElement).toBe(el)` for focus assertions.

## 2025-05-23 - Actionable Empty States
**Learning:** Empty states (like "No transcript segments") should be actionable, not just informational. Providing direct buttons to common starting actions (e.g., "Live Recording", "Batch Import") significantly improves the "first run" experience and guides the user.
**Action:** When implementing empty states, always ask "What should the user do next?" and provide a button for it. Use `role="img"` and `aria-hidden="true"` for decorative empty state illustrations.
