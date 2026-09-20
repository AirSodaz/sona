// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { apiServerClient } from '../apiServerClient';

describe('apiServerClient', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  afterEach(() => {
    localStorage.clear();
  });

  it('sets and gets base url with trailing slash stripped', () => {
    apiServerClient.setBaseUrl('http://192.168.1.100:14200///');
    expect(apiServerClient.getBaseUrl()).toBe('http://192.168.1.100:14200');
    expect(localStorage.getItem('sona_api_server_url')).toBe('http://192.168.1.100:14200');
  });

  it('sets and gets api key and persists to localStorage', () => {
    apiServerClient.setApiKey('test-secret-token');
    expect(apiServerClient.getApiKey()).toBe('test-secret-token');
    expect(localStorage.getItem('sona_api_server_key')).toBe('test-secret-token');

    apiServerClient.setApiKey('');
    expect(apiServerClient.getApiKey()).toBe('');
    expect(localStorage.getItem('sona_api_server_key')).toBeNull();
  });

  it('constructs audio url with encoded token query param when api key is present', () => {
    apiServerClient.setBaseUrl('http://127.0.0.1:14200');
    apiServerClient.setApiKey('key with special&chars=1');
    const audioUrl = apiServerClient.getAudioUrl('job-123');
    expect(audioUrl).toBe(
      'http://127.0.0.1:14200/v1/transcriptions/job-123/audio?token=key%20with%20special%26chars%3D1'
    );

    apiServerClient.setApiKey('');
    const plainAudioUrl = apiServerClient.getAudioUrl('job-123');
    expect(plainAudioUrl).toBe('http://127.0.0.1:14200/v1/transcriptions/job-123/audio');
  });
});
