## 2025-05-23 - Keyboard Navigation for Dropdown Menus
**Learning:** For custom dropdown menus using `role="menu"`, items should use `tabindex="-1"` to remove them from the natural tab sequence, allowing focus to be managed programmatically via Arrow keys. This prevents Tab from navigating through every menu item, which is the expected behavior for application menus. Also, Vitest setup lacks `jest-dom`, so standard matchers like `toBeInTheDocument` are unavailable.
**Action:** When implementing menus, implement `onKeyDown` for Arrows/Escape/Home/End and use `tabindex="-1"` on items. Use `expect(document.activeElement).toBe(el)` for focus assertions.

## 2025-05-24 - Hiding Decorative Icons
**Learning:** Decorative icons, including those from libraries like `lucide-react`, must explicitly have `aria-hidden="true"` to prevent screen readers from announcing them as images or reading their filenames. For inline SVGs, extracting them to `src/components/Icons.tsx` improves maintainability and ensures consistent accessibility attributes.
**Action:** Always add `aria-hidden="true"` to icon components in `Icons.tsx` and pass it as a prop to external icon components when they are decorative.

## 2025-05-24 - Accessible Sliders with Direct DOM Updates
**Learning:** Components like `SeekSlider` that bypass React renders for performance using `ref` and store subscriptions must also manually update ARIA attributes (`aria-valuenow`, `aria-valuetext`) on the DOM element within the subscription callback. Relying on React props for these attributes is insufficient because the component doesn't re-render during high-frequency updates (e.g., audio playback).
**Action:** In `store.subscribe` callbacks for slider-like components, always update `aria-valuenow` and `aria-valuetext` alongside the `value` property on the input element.
