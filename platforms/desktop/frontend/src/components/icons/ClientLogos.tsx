import type React from 'react';
import antigravitySvg from '../../assets/clients/antigravity.svg';
import cursorSvg from '../../assets/clients/cursor.svg';
import mcpSvg from '../../assets/clients/mcp.svg';
import piOmpSvg from '../../assets/clients/pi-omp.svg';
import windsurfSvg from '../../assets/clients/windsurf.svg';
import anthropicSvg from '../../assets/providers/anthropic.svg';
import openaiSvg from '../../assets/providers/openai.svg';

export interface ClientLogoMeta {
  src: string;
  monochrome?: boolean;
}

export const CLIENT_ICON_MAP: Record<string, ClientLogoMeta> = {
  'claude-desktop': { src: anthropicSvg },
  cursor: { src: cursorSvg, monochrome: true },
  codex: { src: openaiSvg, monochrome: true },
  antigravity: { src: antigravitySvg, monochrome: true },
  'claude-code': { src: anthropicSvg },
  'pi-omp': { src: piOmpSvg, monochrome: true },
  windsurf: { src: windsurfSvg },
  generic: { src: mcpSvg, monochrome: true },
};

export interface ClientLogoProps extends React.ImgHTMLAttributes<HTMLImageElement> {
  clientId?: string | null;
  label?: string;
  size?: number;
  className?: string;
}

export function ClientLogo({
  clientId,
  label = '',
  size = 18,
  className = '',
  alt,
  ...props
}: ClientLogoProps): React.JSX.Element {
  const iconEntry = clientId ? CLIENT_ICON_MAP[clientId] : null;

  if (iconEntry) {
    return (
      <img
        src={iconEntry.src}
        alt={alt !== undefined ? alt : label || clientId || 'Client logo'}
        width={size}
        height={size}
        className={`mcp-client-logo ${iconEntry.monochrome ? 'mcp-client-logo-monochrome' : ''} ${className}`.trim()}
        style={{ width: `${size}px`, height: `${size}px` }}
        loading="lazy"
        decoding="async"
        {...props}
      />
    );
  }

  const initial = (label || clientId || '?').charAt(0).toUpperCase();

  return (
    <span
      className={`client-picker-fallback-icon ${className}`.trim()}
      style={{
        width: `${size}px`,
        height: `${size}px`,
        fontSize: `${Math.max(10, Math.round(size * 0.42))}px`,
      }}
      aria-hidden="true"
    >
      {initial}
    </span>
  );
}

export default ClientLogo;
