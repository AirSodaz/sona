import { describe, expect, it } from 'vitest';
import {
  inferAudioExtensionFromBlob,
  inferAudioExtensionFromMime,
  inferAudioExtensionFromPath,
} from '../historyTransportUtils';

describe('historyTransportUtils audio extension inference', () => {
  it('infers audio extension from mime type strings', () => {
    expect(inferAudioExtensionFromMime('audio/wav')).toBe('wav');
    expect(inferAudioExtensionFromMime('audio/wave')).toBe('wav');
    expect(inferAudioExtensionFromMime('audio/webm;codecs=opus')).toBe('webm');
    expect(inferAudioExtensionFromMime('audio/mp4')).toBe('m4a');
    expect(inferAudioExtensionFromMime('audio/aac')).toBe('aac');
    expect(inferAudioExtensionFromMime('audio/ogg')).toBe('ogg');
    expect(inferAudioExtensionFromMime('audio/flac')).toBe('flac');
    expect(inferAudioExtensionFromMime('audio/mpeg')).toBe('mp3');
    expect(inferAudioExtensionFromMime('audio/opus')).toBe('opus');
    expect(inferAudioExtensionFromMime('application/octet-stream')).toBeNull();
  });

  it('infers audio extension from Blob', () => {
    const webmBlob = new Blob([], { type: 'audio/webm' });
    expect(inferAudioExtensionFromBlob(webmBlob)).toBe('webm');

    const m4aBlob = new Blob([], { type: 'audio/mp4' });
    expect(inferAudioExtensionFromBlob(m4aBlob)).toBe('m4a');

    const unknownBlob = new Blob([], { type: 'application/unknown' });
    expect(inferAudioExtensionFromBlob(unknownBlob, 'fallback_ext')).toBe('fallback_ext');
  });

  it('infers audio extension from file paths', () => {
    expect(inferAudioExtensionFromPath('C:/path/to/record.wav')).toBe('wav');
    expect(inferAudioExtensionFromPath('/Users/audio/file.M4A')).toBe('m4a');
    expect(inferAudioExtensionFromPath('relative\\sound.opus')).toBe('opus');
    expect(inferAudioExtensionFromPath('no_extension', 'default')).toBe('default');
    expect(inferAudioExtensionFromPath('trailing_dot.', 'default')).toBe('default');
  });
});
