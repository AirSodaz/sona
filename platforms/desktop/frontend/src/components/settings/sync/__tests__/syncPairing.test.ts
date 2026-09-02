import { describe, expect, it } from 'vitest';
import { decodeSyncPairingToken, encodeSyncPairingToken } from '../syncPairing';

describe('syncPairing', () => {
  const sampleProvider = {
    serverUrl: 'https://dav.jianguoyun.com/dav/',
    remoteRoot: 'Sona',
    username: 'user@example.com',
    password: 'app-password-123',
  };
  const sampleVaultId = 'vault-uuid-456';

  it('encodes and decodes pairing tokens successfully', () => {
    const token = encodeSyncPairingToken(sampleProvider, sampleVaultId, true);
    expect(token).toMatch(/^sonasync:\/\/v1\?data=/);

    const decoded = decodeSyncPairingToken(token);
    expect(decoded).not.toBeNull();
    expect(decoded?.serverUrl).toBe('https://dav.jianguoyun.com/dav/');
    expect(decoded?.remoteRoot).toBe('Sona');
    expect(decoded?.username).toBe('user@example.com');
    expect(decoded?.vaultId).toBe('vault-uuid-456');
    expect(decoded?.providerPassword).toBe('app-password-123');
  });

  it('encodes without provider password when requested', () => {
    const token = encodeSyncPairingToken(sampleProvider, sampleVaultId, false);
    const decoded = decodeSyncPairingToken(token);
    expect(decoded).not.toBeNull();
    expect(decoded?.providerPassword).toBe('');
    expect(decoded?.vaultId).toBe('vault-uuid-456');
  });

  it('returns null on invalid token strings', () => {
    expect(decodeSyncPairingToken('')).toBeNull();
    expect(decodeSyncPairingToken('invalid-token')).toBeNull();
    expect(decodeSyncPairingToken('sonasync://v1?data=invalid_base64_???')).toBeNull();
    expect(decodeSyncPairingToken('sonasync://v1?data=' + btoa('{"invalid": true}'))).toBeNull();
  });
});
