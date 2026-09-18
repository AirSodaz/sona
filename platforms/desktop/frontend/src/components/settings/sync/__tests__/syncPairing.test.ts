import { describe, expect, it } from 'vitest';
import {
  decodeSyncPairingToken,
  encodeS3SyncPairingToken,
  encodeSyncPairingToken,
  isS3PairingPayload,
  isWebDavPairingPayload,
} from '../syncPairing';

describe('syncPairing', () => {
  const sampleProvider = {
    serverUrl: 'https://dav.jianguoyun.com/dav/',
    remoteRoot: 'Sona',
    username: 'user@example.com',
    password: 'app-password-123',
  };
  const sampleVaultId = 'vault-uuid-456';

  const sampleS3Provider = {
    endpoint: 'https://r2.cloudflarestorage.com',
    region: 'auto',
    bucket: 'my-bucket',
    remoteRoot: 'sona',
    accessKeyId: 'ak-123',
    secretAccessKey: 'sk-456',
    forcePathStyle: false,
  };

  it('encodes and decodes WebDAV pairing tokens successfully', () => {
    const token = encodeSyncPairingToken(sampleProvider, sampleVaultId, true);
    expect(token).toMatch(/^sonasync:\/\/v1\?data=/);

    const decoded = decodeSyncPairingToken(token);
    expect(decoded).not.toBeNull();
    if (decoded && isWebDavPairingPayload(decoded)) {
      expect(decoded.providerId).toBe('webdav');
      expect(decoded.serverUrl).toBe('https://dav.jianguoyun.com/dav/');
      expect(decoded.remoteRoot).toBe('Sona');
      expect(decoded.username).toBe('user@example.com');
      expect(decoded.vaultId).toBe('vault-uuid-456');
      expect(decoded.providerPassword).toBe('app-password-123');
    } else {
      expect.unreachable('expected WebDAV payload');
    }
  });

  it('encodes without provider password when requested', () => {
    const token = encodeSyncPairingToken(sampleProvider, sampleVaultId, false);
    const decoded = decodeSyncPairingToken(token);
    expect(decoded).not.toBeNull();
    if (decoded && isWebDavPairingPayload(decoded)) {
      expect(decoded.providerPassword).toBe('');
      expect(decoded.vaultId).toBe('vault-uuid-456');
    }
  });

  it('encodes and decodes S3 pairing tokens successfully', () => {
    const token = encodeS3SyncPairingToken(sampleS3Provider, sampleVaultId, true);
    expect(token).toMatch(/^sonasync:\/\/v2\?data=/);

    const decoded = decodeSyncPairingToken(token);
    expect(decoded).not.toBeNull();
    if (decoded && isS3PairingPayload(decoded)) {
      expect(decoded.providerId).toBe('s3');
      expect(decoded.endpoint).toBe('https://r2.cloudflarestorage.com');
      expect(decoded.region).toBe('auto');
      expect(decoded.bucket).toBe('my-bucket');
      expect(decoded.remoteRoot).toBe('sona');
      expect(decoded.accessKeyId).toBe('ak-123');
      expect(decoded.secretAccessKey).toBe('sk-456');
      expect(decoded.vaultId).toBe('vault-uuid-456');
      expect(decoded.forcePathStyle).toBe(false);
    } else {
      expect.unreachable('expected S3 payload');
    }
  });

  it('encodes S3 pairing token without secret key when requested', () => {
    const token = encodeS3SyncPairingToken(sampleS3Provider, sampleVaultId, false);
    const decoded = decodeSyncPairingToken(token);
    expect(decoded).not.toBeNull();
    if (decoded && isS3PairingPayload(decoded)) {
      expect(decoded.secretAccessKey).toBe('');
      expect(decoded.vaultId).toBe('vault-uuid-456');
    }
  });

  it('returns null on invalid token strings', () => {
    expect(decodeSyncPairingToken('')).toBeNull();
    expect(decodeSyncPairingToken('invalid-token')).toBeNull();
    expect(decodeSyncPairingToken('sonasync://v1?data=invalid_base64_???')).toBeNull();
    expect(decodeSyncPairingToken(`sonasync://v1?data=${btoa('{"invalid": true}')}`)).toBeNull();
  });
});
