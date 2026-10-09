import { describe, expect, it, vi } from 'vitest';
import {
  BrowserAppLifecyclePort,
  BrowserAssetPort,
  BrowserDialogPort,
  BrowserPathPort,
  createWebPlatform,
  HttpApiTransport,
  UnsupportedCapabilityError,
  WebSocketApiTransport,
} from '../drivers/web';

describe('HttpApiTransport', () => {
  it('invokes supported routes via HTTP fetch with JSON serialization', async () => {
    const mockFetch = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      json: async () => ({ status: 'ok', uptime: 100 }),
    });

    const transport = new HttpApiTransport({
      baseUrl: 'http://localhost:14200',
      apiKey: 'secret-token',
      fetchFn: mockFetch,
    });

    const health = await transport.invoke<{ status: string }>('v1/health');
    expect(health).toEqual({ status: 'ok', uptime: 100 });
    expect(mockFetch).toHaveBeenCalledWith(
      'http://localhost:14200/health',
      expect.objectContaining({
        method: 'GET',
        headers: expect.objectContaining({
          Authorization: 'Bearer secret-token',
        }),
      })
    );

    const transcribeResult = await transport.invoke('v1/transcriptions', { file: 'audio.wav' });
    expect(transcribeResult).toEqual({ status: 'ok', uptime: 100 });
    expect(mockFetch).toHaveBeenCalledWith(
      'http://localhost:14200/v1/transcriptions',
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({ file: 'audio.wav' }),
      })
    );
  });

  it('throws UnsupportedCapabilityError for desktop-only commands', async () => {
    const transport = new HttpApiTransport();

    await expect(transport.invoke('history_list_items')).rejects.toThrow(
      UnsupportedCapabilityError
    );

    try {
      await transport.invoke('save_app_config');
    } catch (err) {
      expect(err).toBeInstanceOf(UnsupportedCapabilityError);
      const capError = err as UnsupportedCapabilityError;
      expect(capError.command).toBe('save_app_config');
      expect(capError.environment).toBe('web');
      expect(capError.code).toBe('UNSUPPORTED_CAPABILITY');
    }
  });

  it('handles in-process listen and emit', async () => {
    const transport = new HttpApiTransport();
    const received: string[] = [];

    const unlisten = transport.listen<string>('custom:event', (payload) => {
      received.push(payload);
    });

    await transport.emit('custom:event', 'event-1');
    expect(received).toEqual(['event-1']);

    unlisten();
    await transport.emit('custom:event', 'event-2');
    expect(received).toEqual(['event-1']);
  });
});

describe('WebSocketApiTransport', () => {
  it('delegates invoke to internal HttpApiTransport', async () => {
    const mockFetch = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      json: async () => ({ result: 'transcribed' }),
    });

    const transport = new WebSocketApiTransport({
      baseUrl: 'http://localhost:14200',
      fetchFn: mockFetch,
    });

    const result = await transport.invoke('v1/transcriptions', { id: '123' });
    expect(result).toEqual({ result: 'transcribed' });
  });
});

describe('BrowserPlatformPorts', () => {
  it('BrowserAssetPort preserves URLs', () => {
    const assetPort = new BrowserAssetPort();
    expect(assetPort.convertAudioSrc('blob:http://localhost/123')).toBe(
      'blob:http://localhost/123'
    );
    expect(assetPort.convertAudioSrc('https://example.com/audio.mp3')).toBe(
      'https://example.com/audio.mp3'
    );
  });

  it('BrowserPathPort normalizes POSIX-like paths', async () => {
    const pathPort = new BrowserPathPort('/custom/web/data', '/custom/web/temp');
    expect(await pathPort.appLocalDataDir()).toBe('/custom/web/data');
    expect(await pathPort.tempDir()).toBe('/custom/web/temp');
    expect(pathPort.join('a', 'b', 'c')).toBe('a/b/c');
  });

  it('BrowserDialogPort saveFile returns default path or download label', async () => {
    const dialog = new BrowserDialogPort();
    expect(await dialog.saveFile({ defaultPath: 'my-file.txt' })).toBe('my-file.txt');
    expect(await dialog.saveFile()).toBe('download');
  });

  it('BrowserAppLifecyclePort openLogFolder throws UnsupportedCapabilityError', async () => {
    const lifecycle = new BrowserAppLifecyclePort();
    await expect(lifecycle.openLogFolder()).rejects.toThrow(UnsupportedCapabilityError);
  });
});

describe('createWebPlatform', () => {
  it('constructs a functional web PlatformContext', () => {
    const webPlatform = createWebPlatform({ baseUrl: 'http://localhost:14200' });
    expect(webPlatform.transport).toBeInstanceOf(WebSocketApiTransport);
    expect(webPlatform.ports.assets).toBeInstanceOf(BrowserAssetPort);
    expect(webPlatform.ports.dialog).toBeInstanceOf(BrowserDialogPort);
    expect(webPlatform.ports.path).toBeInstanceOf(BrowserPathPort);
    expect(webPlatform.ports.lifecycle).toBeInstanceOf(BrowserAppLifecyclePort);
  });
});
