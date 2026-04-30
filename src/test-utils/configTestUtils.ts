import { DEFAULT_CONFIG, useConfigStore } from '../stores/configStore';
import { useEffectiveConfigStore } from '../stores/effectiveConfigStore';
import type { AppConfig } from '../types/config';

type Primitive = string | number | boolean | bigint | symbol | null | undefined;

export type DeepPartial<T> =
  T extends Primitive ? T
    : T extends Array<infer U> ? DeepPartial<U>[]
      : T extends object ? { [K in keyof T]?: DeepPartial<T[K]> }
        : T;

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return Object.prototype.toString.call(value) === '[object Object]';
}

function cloneValue<T>(value: T): T {
  if (Array.isArray(value)) {
    return value.map((item) => cloneValue(item)) as T;
  }

  if (isPlainObject(value)) {
    const clonedEntries = Object.entries(value).map(([key, entryValue]) => [key, cloneValue(entryValue)]);
    return Object.fromEntries(clonedEntries) as T;
  }

  return value;
}

function mergePlainObject<T extends Record<string, unknown>>(
  base: T,
  overrides: Record<string, unknown>,
): T {
  const result = cloneValue(base) as Record<string, unknown>;

  for (const [key, overrideValue] of Object.entries(overrides)) {
    const baseValue = result[key];
    result[key] = isPlainObject(baseValue) && isPlainObject(overrideValue)
      ? mergePlainObject(baseValue, overrideValue)
      : cloneValue(overrideValue);
  }

  return result as T;
}

export function buildTestConfig(overrides: DeepPartial<AppConfig> = {}): AppConfig {
  return mergePlainObject(
    DEFAULT_CONFIG as unknown as Record<string, unknown>,
    overrides as Record<string, unknown>,
  ) as unknown as AppConfig;
}

export function setTestConfig(overrides: DeepPartial<AppConfig> = {}): AppConfig {
  const config = buildTestConfig(overrides);
  useConfigStore.setState((state) => ({
    ...state,
    config,
  }));
  useEffectiveConfigStore.getState().syncConfig();
  return config;
}
