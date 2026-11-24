import type React from 'react';

/**
 * Shared window mockup elements for light theme.
 */
function LightWindow(): React.JSX.Element {
  return (
    <>
      <rect x="30" y="16" width="90" height="84" rx="6" fill="#000000" opacity="0.04" />
      <path
        d="M 30,100 L 30,21 A 6 6 0 0 1 36,15 L 114,15 A 6 6 0 0 1 120,21 L 120,100 Z"
        fill="#ffffff"
      />
      <path d="M 93,15 L 114,15 A 6 6 0 0 1 120,21 L 120,100 L 93,100 Z" fill="#f0f0f0" />
      <line x1="93" y1="15" x2="93" y2="100" stroke="#e5e5e5" strokeWidth="1" />
      <rect x="40" y="26" width="22" height="4" rx="2" fill="#e5e5e5" />
      <rect x="40" y="36" width="42" height="4" rx="2" fill="#e5e5e5" />
    </>
  );
}

/**
 * Shared window mockup elements for dark theme.
 */
function DarkWindow(): React.JSX.Element {
  return (
    <>
      <rect x="30" y="16" width="90" height="84" rx="6" fill="#000000" opacity="0.3" />
      <path
        d="M 30,100 L 30,21 A 6 6 0 0 1 36,15 L 114,15 A 6 6 0 0 1 120,21 L 120,100 Z"
        fill="#2a2a2a"
      />
      <path d="M 93,15 L 114,15 A 6 6 0 0 1 120,21 L 120,100 L 93,100 Z" fill="#202020" />
      <line x1="93" y1="15" x2="93" y2="100" stroke="#333333" strokeWidth="1" />
      <rect x="40" y="26" width="22" height="4" rx="2" fill="#3e3e3e" />
      <rect x="40" y="36" width="42" height="4" rx="2" fill="#3e3e3e" />
    </>
  );
}

export function ThemePreviewLight(): React.JSX.Element {
  return (
    <svg viewBox="0 0 150 100" preserveAspectRatio="none" aria-hidden="true">
      <rect x="0" y="0" width="150" height="100" fill="#f5f5f5" />
      <LightWindow />
    </svg>
  );
}

export function ThemePreviewDark(): React.JSX.Element {
  return (
    <svg viewBox="0 0 150 100" preserveAspectRatio="none" aria-hidden="true">
      <rect x="0" y="0" width="150" height="100" fill="#191919" />
      <DarkWindow />
    </svg>
  );
}

export function ThemePreviewAuto(): React.JSX.Element {
  return (
    <svg viewBox="0 0 150 100" preserveAspectRatio="none" aria-hidden="true">
      <defs>
        <clipPath id="sona-theme-auto-clip-left">
          <rect x="-5" y="-5" width="80" height="110" />
        </clipPath>
        <clipPath id="sona-theme-auto-clip-right">
          <rect x="75" y="-5" width="80" height="110" />
        </clipPath>
      </defs>

      {/* Discrete backgrounds for left (light) and right (dark) */}
      <rect x="0" y="0" width="75" height="100" fill="#f5f5f5" />
      <rect x="75" y="0" width="75" height="100" fill="#191919" />

      {/* Right half: Dark content */}
      <g clipPath="url(#sona-theme-auto-clip-right)">
        <DarkWindow />
      </g>

      {/* Left half: Light content */}
      <g clipPath="url(#sona-theme-auto-clip-left)">
        <LightWindow />
      </g>
    </svg>
  );
}
