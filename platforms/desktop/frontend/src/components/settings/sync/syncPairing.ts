import type { WebDavObjectStoreConfig } from '../../../types/sync';

export interface SyncPairingPayload {
  v: 1;
  serverUrl: string;
  remoteRoot: string;
  username: string;
  vaultId: string;
  providerPassword?: string;
}

export function encodeSyncPairingToken(
  provider: WebDavObjectStoreConfig,
  vaultId: string,
  includeProviderPassword = true,
): string {
  const payload: SyncPairingPayload = {
    v: 1,
    serverUrl: provider.serverUrl.trim(),
    remoteRoot: provider.remoteRoot.trim(),
    username: provider.username.trim(),
    vaultId: vaultId.trim(),
    ...(includeProviderPassword && provider.password ? { providerPassword: provider.password } : {}),
  };

  const json = JSON.stringify(payload);
  const bytes = new TextEncoder().encode(json);
  let binary = '';
  for (let i = 0; i < bytes.byteLength; i++) {
    binary += String.fromCharCode(bytes[i]);
  }
  const base64 = btoa(binary);
  return `sonasync://v1?data=${encodeURIComponent(base64)}`;
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
    const parsed = JSON.parse(json) as Partial<SyncPairingPayload>;

    if (
      parsed &&
      parsed.v === 1 &&
      typeof parsed.serverUrl === 'string' &&
      typeof parsed.remoteRoot === 'string' &&
      typeof parsed.username === 'string' &&
      typeof parsed.vaultId === 'string'
    ) {
      return {
        v: 1,
        serverUrl: parsed.serverUrl,
        remoteRoot: parsed.remoteRoot,
        username: parsed.username,
        vaultId: parsed.vaultId,
        providerPassword: typeof parsed.providerPassword === 'string' ? parsed.providerPassword : '',
      };
    }
    return null;
  } catch {
    return null;
  }
}
