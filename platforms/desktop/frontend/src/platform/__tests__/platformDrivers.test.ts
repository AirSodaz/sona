import { describe, expect, it } from 'vitest';
import { getPlatform, resetPlatform, setPlatform } from '../context';
import {
  createMockPlatform,
  InMemoryFileSystemPort,
  MockAppLifecyclePort,
  MockAssetPort,
  MockDialogPort,
  MockPathPort,
  MockTransport,
} from '../drivers/mock';

describe('MockTransport', () => {
  it('registers and invokes command handlers', async () => {
    const transport = new MockTransport();
    transport.setCommandHandler('test/ping', async (args) => ({ pong: args }));

    const result = await transport.invoke<{ pong: { msg: string } }>('test/ping', {
      msg: 'hello',
    });

    expect(result).toEqual({ pong: { msg: 'hello' } });
    expect(transport.getInvocations()).toHaveLength(1);
    expect(transport.getInvocations()[0]).toMatchObject({
      command: 'test/ping',
      args: { msg: 'hello' },
    });
  });

  it('falls back to default command handler if configured', async () => {
    const transport = new MockTransport();
    transport.setDefaultCommandHandler(async (command, args) => ({
      handled: command,
      echo: args,
    }));

    const result = await transport.invoke('any/command', { value: 42 });
    expect(result).toEqual({ handled: 'any/command', echo: { value: 42 } });
  });

  it('throws when invoking an unhandled command with no default handler', async () => {
    const transport = new MockTransport();
    await expect(transport.invoke('unhandled/cmd')).rejects.toThrow(
      '[MockTransport] Unhandled command: "unhandled/cmd"'
    );
  });

  it('handles event subscription, emission and unsubscription', () => {
    const transport = new MockTransport();
    const received: string[] = [];

    const unlisten = transport.listen<string>('test:event', (payload) => {
      received.push(payload);
    });

    expect(transport.hasListener('test:event')).toBe(true);

    transport.mockEmit('test:event', 'first');
    expect(received).toEqual(['first']);

    unlisten();
    expect(transport.hasListener('test:event')).toBe(false);

    transport.mockEmit('test:event', 'second');
    expect(received).toEqual(['first']);
  });

  it('records outgoing emit calls', async () => {
    const transport = new MockTransport();
    await transport.emit('outbound:event', { data: 123 });

    expect(transport.getEmissions()).toHaveLength(1);
    expect(transport.getEmissions()[0]).toMatchObject({
      event: 'outbound:event',
      payload: { data: 123 },
    });
  });
});

describe('MockPlatformPorts', () => {
  it('InMemoryFileSystemPort handles file lifecycle', async () => {
    const fs = new InMemoryFileSystemPort();

    expect(await fs.exists('/foo/bar.txt')).toBe(false);

    await fs.writeText('/foo/bar.txt', 'hello world');
    expect(await fs.exists('/foo/bar.txt')).toBe(true);
    expect(await fs.readText('/foo/bar.txt')).toBe('hello world');

    const binary = new TextEncoder().encode('binary data');
    await fs.writeFile('/foo/bin.dat', binary);
    expect(await fs.readFile('/foo/bin.dat')).toEqual(binary);

    await fs.remove('/foo/bar.txt');
    expect(await fs.exists('/foo/bar.txt')).toBe(false);
  });

  it('MockAssetPort converts path to mock uri', () => {
    const assetPort = new MockAssetPort();
    expect(assetPort.convertAudioSrc('/path/to/audio.wav')).toBe('mock-asset:///path/to/audio.wav');
  });

  it('MockDialogPort tracks and returns preset results', async () => {
    const dialog = new MockDialogPort();
    dialog.setNextOpenResult(['/path/file1.wav']);
    dialog.setNextSaveResult('/path/save.wav');

    const openResult = await dialog.openFile({ multiple: true });
    expect(openResult).toEqual(['/path/file1.wav']);

    const saveResult = await dialog.saveFile();
    expect(saveResult).toBe('/path/save.wav');

    expect(dialog.getCalls()).toHaveLength(2);
  });

  it('MockPathPort handles path join and directories', async () => {
    const pathPort = new MockPathPort('/custom/data', '/custom/temp');
    expect(await pathPort.appLocalDataDir()).toBe('/custom/data');
    expect(await pathPort.tempDir()).toBe('/custom/temp');
    expect(pathPort.join('a', 'b', 'c.txt')).toBe('a/b/c.txt');
    expect(pathPort.join('/root/', '/folder/', 'file.txt')).toBe('/root/folder/file.txt');
  });

  it('MockAppLifecyclePort records lifecycle operations', async () => {
    const lifecycle = new MockAppLifecyclePort();
    await lifecycle.openUrl('https://example.com');
    await lifecycle.relaunch();
    await lifecycle.openLogFolder();

    expect(lifecycle.openedUrls).toEqual(['https://example.com']);
    expect(lifecycle.relaunchCount).toBe(1);
    expect(lifecycle.openLogFolderCount).toBe(1);
  });
});

describe('PlatformContext', () => {
  it('allows overriding and resetting platform context', () => {
    const mockPlatform = createMockPlatform();
    setPlatform(mockPlatform);

    expect(getPlatform()).toBe(mockPlatform);

    resetPlatform();
  });
});
