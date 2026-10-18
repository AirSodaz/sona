import { describe, expect, it, vi } from 'vitest';
import type { ITransport } from '../../platform';
import { createBrowserPlatformPorts, UnsupportedCapabilityError } from '../../platform';
import { createShimService } from '../shimService';

describe('ShimService', () => {
  it('detects unsupported environment when Tauri globals are absent', () => {
    const service = createShimService();
    expect(service.isSupported()).toBe(false);
  });

  it('detects supported environment when custom platform context is injected', () => {
    const mockTransport: ITransport = {
      invoke: vi.fn(),
      listen: vi.fn(),
      emit: vi.fn(),
    };
    const service = createShimService({
      transport: mockTransport,
      ports: createBrowserPlatformPorts(),
    });
    expect(service.isSupported()).toBe(true);
  });

  it('getStatus returns null in unsupported environment', async () => {
    const service = createShimService();
    const status = await service.getStatus();
    expect(status).toBeNull();
  });

  it('mutation methods throw UnsupportedCapabilityError in unsupported environment', async () => {
    const service = createShimService();
    await expect(service.install()).rejects.toThrow(UnsupportedCapabilityError);
    await expect(service.uninstall()).rejects.toThrow(UnsupportedCapabilityError);
    await expect(service.setDefaultChannel('nightly')).rejects.toThrow(UnsupportedCapabilityError);
  });

  it('dispatches commands via injected transport when supported', async () => {
    const invoke = vi.fn().mockResolvedValue(undefined);
    const mockTransport: ITransport = {
      invoke,
      listen: vi.fn(),
      emit: vi.fn(),
    };
    const service = createShimService({
      transport: mockTransport,
      ports: createBrowserPlatformPorts(),
    });

    await service.install();
    expect(invoke).toHaveBeenCalledWith('install_shim');

    await service.uninstall();
    expect(invoke).toHaveBeenCalledWith('uninstall_shim');

    await service.setDefaultChannel('stable');
    expect(invoke).toHaveBeenCalledWith('set_default_channel', { channel: 'stable' });
  });
});
