// @vitest-environment jsdom
import { renderHook } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { useApiServerConfig, useConfigStore } from '../configStore';

describe('configStore - useApiServerConfig', () => {
  it('includes httpServerEnabled in useApiServerConfig selector', () => {
    useConfigStore.setState((state) => ({
      config: {
        ...state.config,
        httpServerEnabled: true,
        httpServerHost: '0.0.0.0',
        httpServerPort: 14200,
      },
    }));

    const { result, rerender } = renderHook(() => useApiServerConfig());
    expect(result.current.httpServerEnabled).toBe(true);
    expect(result.current.httpServerHost).toBe('0.0.0.0');
    expect(result.current.httpServerPort).toBe(14200);

    useConfigStore.setState((state) => ({
      config: {
        ...state.config,
        httpServerEnabled: false,
      },
    }));

    rerender();
    expect(result.current.httpServerEnabled).toBe(false);
  });
});
