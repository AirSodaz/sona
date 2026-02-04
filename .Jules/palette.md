## 2025-05-23 - Keyboard Navigation for Dropdown Menus
**Learning:** For custom dropdown menus using `role="menu"`, items should use `tabindex="-1"` to remove them from the natural tab sequence, allowing focus to be managed programmatically via Arrow keys. This prevents Tab from navigating through every menu item, which is the expected behavior for application menus. Also, Vitest setup lacks `jest-dom`, so standard matchers like `toBeInTheDocument` are unavailable.
**Action:** When implementing menus, implement `onKeyDown` for Arrows/Escape/Home/End and use `tabindex="-1"` on items. Use `expect(document.activeElement).toBe(el)` for focus assertions.

## 2025-05-24 - Hiding Decorative Icons
**Learning:** Decorative icons, including those from libraries like `lucide-react`, must explicitly have `aria-hidden="true"` to prevent screen readers from announcing them as images or reading their filenames. For inline SVGs, extracting them to `src/components/Icons.tsx` improves maintainability and ensures consistent accessibility attributes.
**Action:** Always add `aria-hidden="true"` to icon components in `Icons.tsx` and pass it as a prop to external icon components when they are decorative.
