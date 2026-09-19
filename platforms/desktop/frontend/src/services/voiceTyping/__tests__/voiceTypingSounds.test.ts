// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { voiceTypingSoundPlayer } from '../voiceTypingSounds';

describe('VoiceTypingSoundPlayer', () => {
  let mockOscillator: any;
  let mockGain: any;
  let mockAudioContextInstance: any;

  beforeEach(() => {
    mockOscillator = {
      type: 'sine',
      frequency: {
        setValueAtTime: vi.fn(),
        exponentialRampToValueAtTime: vi.fn(),
      },
      connect: vi.fn(),
      start: vi.fn(),
      stop: vi.fn(),
    };

    mockGain = {
      gain: {
        setValueAtTime: vi.fn(),
        exponentialRampToValueAtTime: vi.fn(),
      },
      connect: vi.fn(),
    };

    mockAudioContextInstance = {
      state: 'running',
      currentTime: 10,
      destination: {},
      createOscillator: vi.fn(() => mockOscillator),
      createGain: vi.fn(() => mockGain),
      resume: vi.fn(),
    };

    class MockAudioContext {
      constructor() {
        return mockAudioContextInstance;
      }
    }

    vi.stubGlobal('AudioContext', MockAudioContext);
    if (typeof window !== 'undefined') {
      (window as any).AudioContext = MockAudioContext;
    }
    voiceTypingSoundPlayer.resetForTest();
    voiceTypingSoundPlayer.setMuted(false);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    if (typeof window !== 'undefined') {
      delete (window as any).AudioContext;
    }
  });

  it('plays start earcon with ascending frequencies', () => {
    voiceTypingSoundPlayer.play('start');

    expect(mockAudioContextInstance.createOscillator).toHaveBeenCalled();
    expect(mockAudioContextInstance.createGain).toHaveBeenCalled();
    expect(mockOscillator.start).toHaveBeenCalledWith(10);
    expect(mockOscillator.frequency.setValueAtTime).toHaveBeenCalledWith(
      expect.closeTo(698.46, 0.1),
      10
    );
  });

  it('plays commit earcon with confirmation frequencies', () => {
    voiceTypingSoundPlayer.play('commit');

    expect(mockAudioContextInstance.createOscillator).toHaveBeenCalled();
    expect(mockOscillator.frequency.setValueAtTime).toHaveBeenCalledWith(880, 10);
  });

  it('plays cancel earcon with descending frequencies', () => {
    voiceTypingSoundPlayer.play('cancel');

    expect(mockAudioContextInstance.createOscillator).toHaveBeenCalled();
    expect(mockOscillator.frequency.setValueAtTime).toHaveBeenCalledWith(
      expect.closeTo(659.25, 0.1),
      10
    );
  });

  it('plays error earcon', () => {
    voiceTypingSoundPlayer.play('error');

    expect(mockAudioContextInstance.createOscillator).toHaveBeenCalled();
    expect(mockOscillator.frequency.setValueAtTime).toHaveBeenCalledWith(
      expect.closeTo(329.63, 0.1),
      10
    );
  });

  it('does not play when muted', () => {
    voiceTypingSoundPlayer.setMuted(true);
    voiceTypingSoundPlayer.play('start');

    expect(mockAudioContextInstance.createOscillator).not.toHaveBeenCalled();
  });
});
