## 2025-03-01 - Form Accessibility in Settings Tabs
**Learning:** In complex configuration panels like `SettingsLLMServiceTab.tsx`, custom React form components and standard `<input>` elements often lack proper explicit DOM linkage. Screen readers cannot infer the association between a floating `<label>` and its adjacent `<input>` unless `htmlFor` and `id` match.
**Action:** When implementing or refactoring settings pages, always ensure inputs have unique `id` attributes (e.g. using a provider prefix like `llm-${def.id}-api-key`) and that their companion `<label>` explicitly links via `htmlFor`. This also improves mouse UX since clicking the label will focus the input.
## 2026-04-20 - Interactive tokens require keyboard accessibility
**Learning:** When making non-semantic HTML elements like `<span>` act as click targets for interactive features (e.g., clickable transcript tokens for seeking playback), they remain completely invisible to keyboard users.
**Action:** Always add `role="button"`, `tabIndex={0}`, and an `onKeyDown` handler that supports 'Enter' and ' ' (Space) to mirror the `onClick` functionality, ensuring full accessibility for screen reader and keyboard-only users.
