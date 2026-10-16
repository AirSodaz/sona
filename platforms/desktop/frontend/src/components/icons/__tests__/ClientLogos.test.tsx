import { render } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { CLIENT_ICON_MAP, ClientLogo } from '../ClientLogos';

describe('ClientLogo', () => {
  it('renders an img element for known clients', () => {
    const { container } = render(
      <ClientLogo clientId="claude-desktop" label="Claude Desktop" size={24} />
    );
    const img = container.querySelector('img');
    expect(img).toBeTruthy();
    expect(img?.getAttribute('alt')).toBe('Claude Desktop');
    expect(img?.getAttribute('width')).toBe('24');
    expect(img?.getAttribute('height')).toBe('24');
    expect(img?.className).toContain('mcp-client-logo');
  });

  it('adds monochrome class for monochrome clients', () => {
    const monochromeClients = ['cursor', 'codex', 'antigravity', 'pi-omp', 'generic'];
    for (const id of monochromeClients) {
      const { container } = render(<ClientLogo clientId={id} label={id} />);
      const img = container.querySelector('img');
      expect(img).toBeTruthy();
      expect(img?.className).toContain('mcp-client-logo-monochrome');
    }
  });

  it('does not add monochrome class for colored brand icons', () => {
    const coloredClients = ['claude-desktop', 'claude-code', 'windsurf'];
    for (const id of coloredClients) {
      const { container } = render(<ClientLogo clientId={id} label={id} />);
      const img = container.querySelector('img');
      expect(img).toBeTruthy();
      expect(img?.className).not.toContain('mcp-client-logo-monochrome');
    }
  });

  it('renders fallback letter-circle for unknown clients', () => {
    const { container } = render(
      <ClientLogo clientId="unknown-client" label="My Custom Agent" size={24} />
    );
    expect(container.querySelector('img')).toBeNull();
    const fallback = container.querySelector('.client-picker-fallback-icon');
    expect(fallback).toBeTruthy();
    expect(fallback?.textContent).toBe('M');
  });

  it('maps all 8 MCP target clients to authentic SVGs', () => {
    const expectedClients = [
      'claude-desktop',
      'cursor',
      'codex',
      'antigravity',
      'claude-code',
      'pi-omp',
      'windsurf',
      'generic',
    ];
    for (const id of expectedClients) {
      const entry = CLIENT_ICON_MAP[id];
      expect(entry, `Client "${id}" should be registered in CLIENT_ICON_MAP`).toBeDefined();
      expect(typeof entry.src).toBe('string');
      expect(entry.src.length).toBeGreaterThan(0);
    }
  });
});
