/**
 * Evaluates whether a host string points to a local or LAN address.
 * Matches the unified policy defined in sona_core::runtime::network.
 */
export function isLocalOrLanHost(host: string): boolean {
  let hostname = host.trim().toLowerCase();
  if (!hostname) return false;

  // Strip enclosing brackets for IPv6 if any
  if (hostname.startsWith('[') && hostname.endsWith(']')) {
    hostname = hostname.slice(1, -1);
  }
  // Strip zone index if any (e.g. fe80::1%eth0)
  if (hostname.includes('%')) {
    hostname = hostname.split('%')[0];
  }
  if (hostname.endsWith('.')) {
    hostname = hostname.slice(0, -1);
  }
  if (!hostname || hostname.startsWith('.')) {
    return false;
  }

  // Check IPv6 (contains ':')
  if (hostname.includes(':')) {
    if (hostname === '::1' || hostname === '::') {
      return true;
    }
    // Unique local fc00::/7 (fc00... or fd00...)
    if (/^[fF][cCdD][0-9a-fA-F]{2}:/.test(hostname)) {
      return true;
    }
    // Link-local fe80::/10 (fe80... to febf...)
    if (/^[fF][eE][89aAbB][0-9a-fA-F]:/.test(hostname)) {
      return true;
    }
    // IPv4-mapped IPv6
    if (hostname.startsWith('::ffff:')) {
      return isLocalOrLanHost(hostname.slice(7));
    }
    return false;
  }

  // Check IPv4
  const ipv4Match = /^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/.exec(hostname);
  if (ipv4Match) {
    const a = Number(ipv4Match[1]);
    const b = Number(ipv4Match[2]);
    const c = Number(ipv4Match[3]);
    const d = Number(ipv4Match[4]);
    if (a > 255 || b > 255 || c > 255 || d > 255) return false;

    // Loopback 127.0.0.0/8, 0.0.0.0, broadcast 255.255.255.255
    if (
      a === 127 ||
      (a === 0 && b === 0 && c === 0 && d === 0) ||
      (a === 255 && b === 255 && c === 255 && d === 255)
    ) {
      return true;
    }
    // Private (RFC 1918): 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16
    if (a === 10 || (a === 172 && b >= 16 && b <= 31) || (a === 192 && b === 168)) {
      return true;
    }
    // Link-local (RFC 3927): 169.254.0.0/16
    if (a === 169 && b === 254) {
      return true;
    }
    // Shared address space / CGNAT (RFC 6598, e.g. Tailscale): 100.64.0.0/10
    if (a === 100 && b >= 64 && b <= 127) {
      return true;
    }
    return false;
  }

  // Hostnames
  if (
    hostname === 'localhost' ||
    hostname === 'localhost.localdomain' ||
    hostname.endsWith('.localhost') ||
    hostname.endsWith('.local') ||
    hostname.endsWith('.localdomain') ||
    hostname.endsWith('.lan') ||
    hostname.endsWith('.home.arpa') ||
    hostname.endsWith('.internal') ||
    !hostname.includes('.')
  ) {
    return true;
  }

  return false;
}

export type SyncUrlValidationError = 'invalid_url' | 'unsupported_scheme' | 'http_not_local';

export function validateSyncServerUrl(urlString: string): {
  valid: boolean;
  error?: SyncUrlValidationError;
} {
  const trimmed = urlString.trim();
  if (!trimmed) {
    return { valid: false, error: 'invalid_url' };
  }
  let url: URL;
  try {
    url = new URL(trimmed);
  } catch {
    return { valid: false, error: 'invalid_url' };
  }

  if (url.protocol === 'https:') {
    return { valid: true };
  }
  if (url.protocol === 'http:') {
    if (isLocalOrLanHost(url.hostname)) {
      return { valid: true };
    }
    return { valid: false, error: 'http_not_local' };
  }

  return { valid: false, error: 'unsupported_scheme' };
}
