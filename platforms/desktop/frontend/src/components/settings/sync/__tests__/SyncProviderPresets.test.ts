import { describe, expect, it } from 'vitest';
import { detectProviderPresetId, detectS3ProviderPresetId } from '../SyncProviderPresets';

describe('detectProviderPresetId', () => {
  it('detects Nutstore by domain or subdomain', () => {
    expect(detectProviderPresetId('https://dav.jianguoyun.com/dav/')).toBe('nutstore');
    expect(detectProviderPresetId('dav.jianguoyun.com')).toBe('nutstore');
  });

  it('detects InfiniCloud by domain or keyword', () => {
    expect(detectProviderPresetId('https://user.teracloud.jp/dav/')).toBe('infinicloud');
    expect(detectProviderPresetId('https://infinicloud.example.com/dav')).toBe('infinicloud');
  });

  it('detects Nextcloud / ownCloud by path or hostname', () => {
    expect(detectProviderPresetId('https://cloud.example.com/remote.php/dav/files/user/')).toBe(
      'nextcloud'
    );
    expect(detectProviderPresetId('https://nextcloud.example.com/dav')).toBe('nextcloud');
    expect(detectProviderPresetId('https://owncloud.example.com/dav')).toBe('nextcloud');
  });

  it('detects Synology by port 5006 or hostname', () => {
    expect(detectProviderPresetId('https://nas.local:5006/')).toBe('synology');
    expect(detectProviderPresetId('https://synology.local/dav')).toBe('synology');
  });

  it('detects Alist by dav path and hostname', () => {
    expect(detectProviderPresetId('https://alist.example.com/dav/')).toBe('alist');
  });

  it('falls back to custom for unknown WebDAV providers', () => {
    expect(detectProviderPresetId('https://dav.example.com/')).toBe('custom');
    expect(detectProviderPresetId('')).toBe('custom');
  });

  it('does not match spoofed domain substrings in pathname or attacker hostnames', () => {
    expect(detectProviderPresetId('https://attacker.com/jianguoyun.com')).toBe('custom');
    expect(detectProviderPresetId('https://attacker-jianguoyun.com/dav')).toBe('custom');
    expect(detectProviderPresetId('https://attacker-teracloud.jp/dav')).toBe('custom');
  });
});

describe('detectS3ProviderPresetId', () => {
  it('detects Cloudflare R2 by domain', () => {
    expect(detectS3ProviderPresetId('https://abc12345.r2.cloudflarestorage.com')).toBe(
      'cloudflare-r2'
    );
    expect(detectS3ProviderPresetId('abc12345.r2.cloudflarestorage.com')).toBe('cloudflare-r2');
  });

  it('detects AWS S3 by domain', () => {
    expect(detectS3ProviderPresetId('https://s3.us-west-2.amazonaws.com')).toBe('aws-s3');
    expect(detectS3ProviderPresetId('https://bucket.s3.amazonaws.com')).toBe('aws-s3');
  });

  it('detects Aliyun OSS by domain', () => {
    expect(detectS3ProviderPresetId('https://oss-cn-hangzhou.aliyuncs.com')).toBe('aliyun-oss');
  });

  it('detects Tencent COS by domain', () => {
    expect(detectS3ProviderPresetId('https://cos.ap-shanghai.myqcloud.com')).toBe('tencent-cos');
  });

  it('detects MinIO by port 9000 or hostname', () => {
    expect(detectS3ProviderPresetId('http://localhost:9000')).toBe('minio');
    expect(detectS3ProviderPresetId('http://192.168.1.100:9000')).toBe('minio');
    expect(detectS3ProviderPresetId('https://minio.corp.internal')).toBe('minio');
  });

  it('falls back to s3-custom for unknown S3 providers', () => {
    expect(detectS3ProviderPresetId('https://s3.example.com')).toBe('s3-custom');
    expect(detectS3ProviderPresetId('')).toBe('s3-custom');
  });

  it('rejects spoofed domain substrings in pathname or attacker hostnames', () => {
    expect(detectS3ProviderPresetId('https://attacker.com/r2.cloudflarestorage.com')).toBe(
      's3-custom'
    );
    expect(detectS3ProviderPresetId('https://attacker.com?endpoint=s3.amazonaws.com')).toBe(
      's3-custom'
    );
    expect(detectS3ProviderPresetId('https://attacker-amazonaws.com')).toBe('s3-custom');
    expect(detectS3ProviderPresetId('https://attacker-aliyuncs.com')).toBe('s3-custom');
    expect(detectS3ProviderPresetId('https://attacker-myqcloud.com')).toBe('s3-custom');
  });
});
