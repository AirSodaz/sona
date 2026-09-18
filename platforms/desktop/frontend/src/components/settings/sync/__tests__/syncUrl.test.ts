import { describe, expect, it } from 'vitest';
import { isLocalOrLanHost, validateSyncServerUrl } from '../syncUrl';

describe('isLocalOrLanHost', () => {
  it('accepts loopback addresses and hostnames', () => {
    for (const host of [
      'localhost',
      'localhost.localdomain',
      'service.localhost',
      '127.0.0.1',
      '127.1.2.3',
      '0.0.0.0',
      '::1',
      '[::1]',
      '::',
    ]) {
      expect(isLocalOrLanHost(host)).toBe(true);
    }
  });

  it('accepts private and link-local IPv4 addresses', () => {
    for (const host of [
      '10.0.0.1',
      '10.255.255.254',
      '172.16.0.1',
      '172.31.255.255',
      '192.168.1.1',
      '192.168.254.10',
      '169.254.1.1',
      '100.64.0.1',
      '100.127.255.254',
    ]) {
      expect(isLocalOrLanHost(host)).toBe(true);
    }
  });

  it('accepts private and link-local IPv6 addresses', () => {
    for (const host of [
      'fc00::1',
      '[fc00::1]',
      'fd12:3456:789a::1',
      '[fd12:3456:789a::1]',
      'fe80::1',
      '[fe80::1]',
      'fe80::1%eth0',
      '[fe80::1%eth0]',
      '::ffff:192.168.1.5',
      '[::ffff:192.168.1.5]',
    ]) {
      expect(isLocalOrLanHost(host)).toBe(true);
    }
  });

  it('accepts LAN hostnames', () => {
    for (const host of [
      'nas',
      'my-pc',
      'nas.local',
      'server.lan',
      'home.home.arpa',
      'nas.localdomain',
      'docker.internal',
      'nas.local.',
    ]) {
      expect(isLocalOrLanHost(host)).toBe(true);
    }
  });

  it('rejects public remote hosts and invalid hostnames', () => {
    for (const host of [
      'example.com',
      'api.openai.com',
      'dav.jianguoyun.com',
      '8.8.8.8',
      '1.1.1.1',
      '100.63.255.255',
      '100.128.0.1',
      '172.15.255.255',
      '172.32.0.1',
      '2001:db8::1',
      '[2001:db8::1]',
      '',
      '   ',
      '.',
      '.local',
    ]) {
      expect(isLocalOrLanHost(host)).toBe(false);
    }
  });
});

describe('validateSyncServerUrl', () => {
  it('accepts https endpoints', () => {
    expect(validateSyncServerUrl('https://dav.jianguoyun.com/dav/').valid).toBe(true);
    expect(validateSyncServerUrl('https://localhost:8080/dav').valid).toBe(true);
  });

  it('accepts local and LAN http endpoints', () => {
    expect(validateSyncServerUrl('http://localhost:8080/dav/').valid).toBe(true);
    expect(validateSyncServerUrl('http://127.0.0.1:8080/dav').valid).toBe(true);
    expect(validateSyncServerUrl('http://nas.local:5005/dav').valid).toBe(true);
    expect(validateSyncServerUrl('http://192.168.1.100:5005/dav').valid).toBe(true);
  });

  it('rejects remote http endpoints', () => {
    const result = validateSyncServerUrl('http://dav.jianguoyun.com/dav');
    expect(result.valid).toBe(false);
    expect(result.error).toBe('http_not_local');
  });

  it('rejects unsupported schemes', () => {
    const result = validateSyncServerUrl('ftp://localhost:8080/dav');
    expect(result.valid).toBe(false);
    expect(result.error).toBe('unsupported_scheme');
  });

  it('rejects invalid urls', () => {
    expect(validateSyncServerUrl('').valid).toBe(false);
    expect(validateSyncServerUrl('not-a-url').valid).toBe(false);
  });
});
