import { createTauriPlatform } from './drivers/tauri';
import type { PlatformContext } from './types';

export type { PlatformContext };

let currentPlatform: PlatformContext | null = null;
let defaultPlatformFactory: (() => PlatformContext) | null = createTauriPlatform;

/**
 * Sets the default factory used to initialize the platform context if one has not been set.
 */
export function setDefaultPlatformFactory(factory: () => PlatformContext): void {
  defaultPlatformFactory = factory;
}

/**
 * Retrieves the current active platform context.
 * If no context has been explicitly set, attempts to initialize via the default platform factory.
 */
export function getPlatform(): PlatformContext {
  if (!currentPlatform) {
    if (defaultPlatformFactory) {
      currentPlatform = defaultPlatformFactory();
    } else {
      throw new Error(
        '[PlatformContext] Platform context has not been initialized. Call setPlatform() or register a default platform factory.'
      );
    }
  }
  return currentPlatform;
}

/**
 * Overrides the active platform context (useful for dependency injection in tests or switching host).
 */
export function setPlatform(ctx: PlatformContext): void {
  currentPlatform = ctx;
}

/**
 * Resets the active platform context back to uninitialized state.
 */
export function resetPlatform(): void {
  currentPlatform = null;
}
