import type { S3ObjectStoreConfig, WebDavObjectStoreConfig } from '../../../types/sync';

export interface WebDavSyncPairingPayload {
  v: 1 | 2;
  providerId: 'webdav';
  serverUrl: string;
  remoteRoot: string;
  username: string;
  vaultId: string;
  providerPassword?: string;
}

export interface S3SyncPairingPayload {
  v: 2;
  providerId: 's3';
  endpoint: string;
  region: string;
  bucket: string;
  remoteRoot: string;
  accessKeyId: string;
  vaultId: string;
  secretAccessKey?: string;
  forcePathStyle?: boolean;
}

export type SyncPairingPayload = WebDavSyncPairingPayload | S3SyncPairingPayload;

export function isS3PairingPayload(payload: SyncPairingPayload): payload is S3SyncPairingPayload {
  return payload.providerId === 's3';
}

export function isWebDavPairingPayload(
  payload: SyncPairingPayload
): payload is WebDavSyncPairingPayload {
  return payload.providerId === 'webdav';
}

export function encodeSyncPairingToken(
  provider: WebDavObjectStoreConfig,
  vaultId: string,
  includeProviderPassword = true
): string {
  const payload: WebDavSyncPairingPayload = {
    v: 1,
    providerId: 'webdav',
    serverUrl: provider.serverUrl.trim(),
    remoteRoot: provider.remoteRoot.trim(),
    username: provider.username.trim(),
    vaultId: vaultId.trim(),
    ...(includeProviderPassword && provider.password
      ? { providerPassword: provider.password }
      : { providerPassword: '' }),
  };

  return encodeTokenObject(payload, 'v1');
}

export function encodeS3SyncPairingToken(
  provider: S3ObjectStoreConfig,
  vaultId: string,
  includeSecretAccessKey = true
): string {
  const payload: S3SyncPairingPayload = {
    v: 2,
    providerId: 's3',
    endpoint: provider.endpoint.trim(),
    region: provider.region.trim(),
    bucket: provider.bucket.trim(),
    remoteRoot: provider.remoteRoot.trim(),
    accessKeyId: provider.accessKeyId.trim(),
    vaultId: vaultId.trim(),
    forcePathStyle: provider.forcePathStyle,
    ...(includeSecretAccessKey && provider.secretAccessKey
      ? { secretAccessKey: provider.secretAccessKey }
      : { secretAccessKey: '' }),
  };

  return encodeTokenObject(payload, 'v2');
}

function encodeTokenObject(payload: unknown, protocolVersion: string): string {
  const json = JSON.stringify(payload);
  const bytes = new TextEncoder().encode(json);
  let binary = '';
  for (let i = 0; i < bytes.byteLength; i++) {
    binary += String.fromCharCode(bytes[i]);
  }
  const base64 = btoa(binary);
  return `sonasync://${protocolVersion}?data=${encodeURIComponent(base64)}`;
}

export function decodeSyncPairingToken(token: string): SyncPairingPayload | null {
  const trimmed = token.trim();
  let base64: string;

  if (trimmed.startsWith('sonasync://')) {
    try {
      const url = new URL(trimmed);
      const dataParam = url.searchParams.get('data');
      if (!dataParam) return null;
      base64 = decodeURIComponent(dataParam);
    } catch {
      // If URL parsing fails, extract data query param manually
      const match = trimmed.match(/[?&]data=([^&#]+)/);
      if (!match) return null;
      base64 = decodeURIComponent(match[1]);
    }
  } else {
    base64 = trimmed;
  }

  try {
    const binary = atob(base64);
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i++) {
      bytes[i] = binary.charCodeAt(i);
    }
    const json = new TextDecoder().decode(bytes);
    const parsed = JSON.parse(json) as Record<string, unknown>;

    if (!parsed || typeof parsed !== 'object') {
      return null;
    }

    // S3 Pairing Payload (v2)
    if (
      parsed.providerId === 's3' &&
      typeof parsed.endpoint === 'string' &&
      typeof parsed.bucket === 'string' &&
      typeof parsed.accessKeyId === 'string' &&
      typeof parsed.vaultId === 'string'
    ) {
      return {
        v: 2,
        providerId: 's3',
        endpoint: parsed.endpoint,
        region: typeof parsed.region === 'string' ? parsed.region : 'us-east-1',
        bucket: parsed.bucket,
        remoteRoot: typeof parsed.remoteRoot === 'string' ? parsed.remoteRoot : 'sona',
        accessKeyId: parsed.accessKeyId,
        vaultId: parsed.vaultId,
        secretAccessKey: typeof parsed.secretAccessKey === 'string' ? parsed.secretAccessKey : '',
        forcePathStyle: Boolean(parsed.forcePathStyle),
      };
    }

    // WebDAV Pairing Payload (v1 or v2)
    if (
      typeof parsed.serverUrl === 'string' &&
      typeof parsed.remoteRoot === 'string' &&
      typeof parsed.username === 'string' &&
      typeof parsed.vaultId === 'string'
    ) {
      return {
        v: parsed.v === 2 ? 2 : 1,
        providerId: 'webdav',
        serverUrl: parsed.serverUrl,
        remoteRoot: parsed.remoteRoot,
        username: parsed.username,
        vaultId: parsed.vaultId,
        providerPassword:
          typeof parsed.providerPassword === 'string' ? parsed.providerPassword : '',
      };
    }

    return null;
  } catch {
    return null;
  }
}
