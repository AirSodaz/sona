import { renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { resetPlatform, setPlatform } from '../context';
import { createMockPlatform } from '../drivers/mock';
import { usePlatform, useTransport } from '../hooks';

describe('Platform React Hooks', () => {
  beforeEach(() => {
    resetPlatform();
  });

  afterEach(() => {
    resetPlatform();
  });

  it('usePlatform returns the active platform context', () => {
    const mockPlatform = createMockPlatform();
    setPlatform(mockPlatform);

    const { result } = renderHook(() => usePlatform());
    expect(result.current).toBe(mockPlatform);
  });

  it('useTransport returns the active platform transport', () => {
    const mockPlatform = createMockPlatform();
    setPlatform(mockPlatform);

    const { result } = renderHook(() => useTransport());
    expect(result.current).toBe(mockPlatform.transport);
  });
});
