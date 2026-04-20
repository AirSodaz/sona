1. **Analyze and Refine `src/services/llmConfig.ts`**
   - The function `extractLegacyModel` contains a deeply nested ternary operator for evaluating `model`.
   - Update it to use an explicit `if/else` chain or check sequence for improved clarity. This directly violates the "Avoid nested ternary operators" rule from CLAUDE.md.
2. **Analyze and Refine `src/utils/errorUtils.ts`**
   - The function `buildErrorDialogViewModel` contains nested ternaries when creating the `primaryLabel` and `cancelLabel` properties within its return block.
   - Separate the ternary resolution into explicit boolean/string declarations before returning the object.
3. **Analyze and Refine `src/utils/onboarding.ts`**
   - Review `getResumeOnboardingStep` and other state resolution functions. There are nested logic structures involving ternaries (e.g., checking `typeof parsed.reminderDismissedAt === 'string'`).
   - Although it's slightly less egregious, explicitly resolving the types in standard variables before assigning will simplify the dictionary.
4. **Pre-commit step**
   - Ensure proper testing, verification, review, and reflection are done.
5. **Submit changes**
   - Commit and submit the code simplifications.
