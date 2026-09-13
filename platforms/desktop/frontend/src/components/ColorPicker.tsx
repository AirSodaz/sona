import { ChevronsUpDown, Pipette } from 'lucide-react';
import type React from 'react';
import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { ModalPortal } from './ModalPortal';

export interface ColorPickerProps {
  isOpen: boolean;
  color: string;
  onChange: (hex: string) => void;
  onClose: () => void;
  onPickingChange?: (isPicking: boolean) => void;
  anchorRef: React.RefObject<HTMLElement | null>;
}

// Color conversion helpers
function hexToRgb(hex: string): { r: number; g: number; b: number } {
  let cleaned = hex.replace('#', '');
  if (cleaned.length === 3) {
    cleaned = cleaned
      .split('')
      .map((c) => c + c)
      .join('');
  }
  if (cleaned.length !== 6) {
    return { r: 99, g: 102, b: 241 };
  }
  return {
    r: parseInt(cleaned.slice(0, 2), 16) || 0,
    g: parseInt(cleaned.slice(2, 4), 16) || 0,
    b: parseInt(cleaned.slice(4, 6), 16) || 0,
  };
}

function rgbToHex(r: number, g: number, b: number): string {
  const toHex = (n: number) => {
    const clamped = Math.max(0, Math.min(255, Math.round(n)));
    const h = clamped.toString(16);
    return h.length === 1 ? `0${h}` : h;
  };
  return `#${toHex(r)}${toHex(g)}${toHex(b)}`.toUpperCase();
}

function rgbToHsv(r: number, g: number, b: number): { h: number; s: number; v: number } {
  const rNorm = r / 255;
  const gNorm = g / 255;
  const bNorm = b / 255;
  const max = Math.max(rNorm, gNorm, bNorm);
  const min = Math.min(rNorm, gNorm, bNorm);
  const d = max - min;
  let h = 0;
  if (d !== 0) {
    if (max === rNorm) h = ((gNorm - bNorm) / d + (gNorm < bNorm ? 6 : 0)) * 60;
    else if (max === gNorm) h = ((bNorm - rNorm) / d + 2) * 60;
    else h = ((rNorm - gNorm) / d + 4) * 60;
  }
  const s = max === 0 ? 0 : (d / max) * 100;
  const v = max * 100;
  return { h: Math.round(h), s: Math.round(s), v: Math.round(v) };
}

function hsvToRgb(h: number, s: number, v: number): { r: number; g: number; b: number } {
  const sNorm = s / 100;
  const vNorm = v / 100;
  const c = vNorm * sNorm;
  const x = c * (1 - Math.abs(((h / 60) % 2) - 1));
  const m = vNorm - c;
  let r = 0;
  let g = 0;
  let b = 0;
  if (h >= 0 && h < 60) {
    r = c;
    g = x;
  } else if (h >= 60 && h < 120) {
    r = x;
    g = c;
  } else if (h >= 120 && h < 180) {
    g = c;
    b = x;
  } else if (h >= 180 && h < 240) {
    g = x;
    b = c;
  } else if (h >= 240 && h < 300) {
    r = x;
    b = c;
  } else {
    r = c;
    b = x;
  }
  return {
    r: Math.max(0, Math.min(255, Math.round((r + m) * 255))),
    g: Math.max(0, Math.min(255, Math.round((g + m) * 255))),
    b: Math.max(0, Math.min(255, Math.round((b + m) * 255))),
  };
}
export function ColorPicker({
  isOpen,
  color,
  onChange,
  onClose,
  onPickingChange,
  anchorRef,
}: ColorPickerProps): React.JSX.Element | null {
  const pickerRef = useRef<HTMLDivElement>(null);
  const boardRef = useRef<HTMLDivElement>(null);
  const hueRef = useRef<HTMLDivElement>(null);
  const fallbackInputRef = useRef<HTMLInputElement>(null);
  const isPickingRef = useRef(false);
  const initialRgb = hexToRgb(color || '#6366F1');
  const [prevColor, setPrevColor] = useState(color);
  const [rgb, setRgb] = useState(initialRgb);
  const [hsv, setHsv] = useState(() => rgbToHsv(initialRgb.r, initialRgb.g, initialRgb.b));
  const [format, setFormat] = useState<'rgb' | 'hex'>('rgb');
  const [hexInput, setHexInput] = useState(() => (color || '#6366F1').toUpperCase());
  const [popoverStyle, setPopoverStyle] = useState<React.CSSProperties>({});

  // Adjust state during render when color prop changes externally
  if (color !== prevColor) {
    setPrevColor(color);
    const nextRgb = hexToRgb(color || '#6366F1');
    setRgb(nextRgb);
    setHsv(rgbToHsv(nextRgb.r, nextRgb.g, nextRgb.b));
    setHexInput((color || '#6366F1').toUpperCase());
  }
  // Position popover relative to anchor
  useLayoutEffect(() => {
    if (isOpen && anchorRef.current) {
      const rect = anchorRef.current.getBoundingClientRect();
      const width = 248;
      const height = 270;

      const spaceBelow = window.innerHeight - rect.bottom;
      const spaceAbove = rect.top;

      let top = rect.bottom + 6;
      if (spaceBelow < height && spaceAbove > spaceBelow) {
        top = Math.max(8, rect.top - height - 6);
      }

      const maxLeft = Math.max(8, window.innerWidth - width - 12);
      const left = Math.max(8, Math.min(rect.left, maxLeft));

      setPopoverStyle({
        position: 'fixed',
        top,
        left,
        width: `${width}px`,
        zIndex: 2600,
      });
    }
  }, [isOpen, anchorRef]);

  // Click outside listener
  useEffect(() => {
    if (!isOpen) return;

    const handleClickOutside = (event: MouseEvent) => {
      if (isPickingRef.current) return;
      const target = event.target as Node;
      if (
        pickerRef.current &&
        !pickerRef.current.contains(target) &&
        anchorRef.current &&
        !anchorRef.current.contains(target)
      ) {
        onClose();
      }
    };

    const handleScroll = (event: Event) => {
      if (isPickingRef.current) return;
      if (pickerRef.current && !pickerRef.current.contains(event.target as Node)) {
        onClose();
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    window.addEventListener('scroll', handleScroll, true);
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
      window.removeEventListener('scroll', handleScroll, true);
    };
  }, [isOpen, onClose, anchorRef]);

  // Saturation / Value board drag
  const updateBoardCoords = (clientX: number, clientY: number) => {
    if (!boardRef.current) return;
    const rect = boardRef.current.getBoundingClientRect();
    const x = Math.max(0, Math.min(rect.width, clientX - rect.left));
    const y = Math.max(0, Math.min(rect.height, clientY - rect.top));
    const s = Math.round((x / rect.width) * 100);
    const v = Math.round((1 - y / rect.height) * 100);

    setHsv((prev) => {
      const next = { ...prev, s, v };
      const nextRgb = hsvToRgb(next.h, next.s, next.v);
      const nextHex = rgbToHex(nextRgb.r, nextRgb.g, nextRgb.b);
      setRgb(nextRgb);
      setHexInput(nextHex);
      onChange(nextHex);
      return next;
    });
  };

  const handleBoardPointerDown = (e: React.PointerEvent) => {
    e.preventDefault();
    e.stopPropagation();
    const target = e.currentTarget;
    try {
      target.setPointerCapture(e.pointerId);
    } catch {
      // ignore
    }
    updateBoardCoords(e.clientX, e.clientY);

    const onPointerMove = (ev: PointerEvent) => {
      updateBoardCoords(ev.clientX, ev.clientY);
    };
    const onPointerUp = (ev: PointerEvent) => {
      try {
        target.releasePointerCapture(ev.pointerId);
      } catch {
        // ignore
      }
      window.removeEventListener('pointermove', onPointerMove);
      window.removeEventListener('pointerup', onPointerUp);
    };

    window.addEventListener('pointermove', onPointerMove);
    window.addEventListener('pointerup', onPointerUp);
  };

  const updateHueCoords = (clientX: number) => {
    if (!hueRef.current) return;
    const rect = hueRef.current.getBoundingClientRect();
    if (rect.width <= 0) return;
    const x = Math.max(0, Math.min(rect.width, clientX - rect.left));
    const rawH = Math.round((x / rect.width) * 360);
    const h = Math.min(360, Math.max(0, rawH)) % 360;

    setHsv((prev) => {
      let s = prev.s;
      let v = prev.v;
      if (s === 0) s = 100;
      if (v === 0) v = 100;
      const next = { ...prev, h, s, v };
      const nextRgb = hsvToRgb(next.h, next.s, next.v);
      const nextHex = rgbToHex(nextRgb.r, nextRgb.g, nextRgb.b);
      setRgb(nextRgb);
      setHexInput(nextHex);
      onChange(nextHex);
      return next;
    });
  };

  const handleHuePointerDown = (e: React.PointerEvent) => {
    e.preventDefault();
    e.stopPropagation();
    const target = e.currentTarget;
    try {
      target.setPointerCapture(e.pointerId);
    } catch {
      // ignore
    }
    updateHueCoords(e.clientX);

    const onPointerMove = (ev: PointerEvent) => {
      updateHueCoords(ev.clientX);
    };
    const onPointerUp = (ev: PointerEvent) => {
      try {
        target.releasePointerCapture(ev.pointerId);
      } catch {
        // ignore
      }
      window.removeEventListener('pointermove', onPointerMove);
      window.removeEventListener('pointerup', onPointerUp);
    };

    window.addEventListener('pointermove', onPointerMove);
    window.addEventListener('pointerup', onPointerUp);
  };

  // EyeDropper API with fallback
  const handleEyeDropper = async (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();

    isPickingRef.current = true;
    onPickingChange?.(true);
    if (typeof window !== 'undefined' && 'EyeDropper' in window) {
      try {
        type EyeDropperInstance = { open: () => Promise<{ sRGBHex: string }> };
        type EyeDropperConstructor = new () => EyeDropperInstance;
        const windowWithEyeDropper = window as unknown as { EyeDropper: EyeDropperConstructor };
        const eyeDropper = new windowWithEyeDropper.EyeDropper();
        const result = await eyeDropper.open();
        if (result?.sRGBHex) {
          const newHex = result.sRGBHex.toUpperCase();
          const newRgb = hexToRgb(newHex);
          setRgb(newRgb);
          setHsv(rgbToHsv(newRgb.r, newRgb.g, newRgb.b));
          setHexInput(newHex);
          onChange(newHex);
        }
      } catch (err: unknown) {
        const isAbort =
          err instanceof Error &&
          (err.name === 'AbortError' || err.message?.toLowerCase().includes('abort'));
        if (!isAbort) {
          try {
            const input = fallbackInputRef.current;
            if (input && typeof input.showPicker === 'function') {
              input.showPicker();
            } else if (input) {
              input.click();
            }
          } catch {
            // Ignore fallback errors
          }
        }
      } finally {
        setTimeout(() => {
          isPickingRef.current = false;
          onPickingChange?.(false);
        }, 350);
      }
    } else {
      try {
        const input = fallbackInputRef.current;
        if (input && typeof input.showPicker === 'function') {
          input.showPicker();
        } else if (input) {
          input.click();
        }
      } catch {
        // Ignore fallback errors
      } finally {
        setTimeout(() => {
          isPickingRef.current = false;
          onPickingChange?.(false);
        }, 350);
      }
    }
  };

  const currentHex = hexInput;

  const handleRgbChange = (channel: 'r' | 'g' | 'b', rawVal: string) => {
    const val = Math.max(0, Math.min(255, parseInt(rawVal, 10) || 0));
    const nextRgb = { ...rgb, [channel]: val };
    const nextHsv = rgbToHsv(nextRgb.r, nextRgb.g, nextRgb.b);
    const nextHex = rgbToHex(nextRgb.r, nextRgb.g, nextRgb.b);
    setRgb(nextRgb);
    setHsv(nextHsv);
    setHexInput(nextHex);
    onChange(nextHex);
  };

  const handleHexInputChange = (val: string) => {
    setHexInput(val);
    const cleaned = val.replace('#', '');
    if (cleaned.length === 6 && /^[0-9A-Fa-f]{6}$/.test(cleaned)) {
      const nextRgb = hexToRgb(val);
      setRgb(nextRgb);
      setHsv(rgbToHsv(nextRgb.r, nextRgb.g, nextRgb.b));
      onChange(`#${cleaned.toUpperCase()}`);
    }
  };

  if (!isOpen) {
    return null;
  }
  return (
    <ModalPortal>
      <div
        ref={pickerRef}
        className="sona-color-picker-popover"
        onMouseDown={(e) => e.stopPropagation()}
        onTouchStart={(e) => e.stopPropagation()}
        style={{
          ...popoverStyle,
          background: 'var(--color-bg-elevated)',
          border: '1px solid var(--color-border)',
          borderRadius: '12px',
          boxShadow: 'var(--shadow-xl)',
          padding: '12px',
          display: 'flex',
          flexDirection: 'column',
          gap: '10px',
          userSelect: 'none',
        }}
      >
        {/* 1. Saturation / Value Gradient Board with Rounded Corners & Theme Gray Border */}
        <div
          ref={boardRef}
          className="sona-color-picker-board"
          onPointerDown={handleBoardPointerDown}
          style={{
            position: 'relative',
            width: '100%',
            height: '124px',
            borderRadius: '8px',
            overflow: 'hidden',
            cursor: 'crosshair',
            backgroundColor: `hsl(${hsv.h}, 100%, 50%)`,
            border: '1px solid var(--color-border)',
            boxSizing: 'border-box',
          }}
        >
          <div
            style={{
              position: 'absolute',
              inset: 0,
              background: 'linear-gradient(to right, #ffffff, transparent)',
            }}
          />
          <div
            style={{
              position: 'absolute',
              inset: 0,
              background: 'linear-gradient(to top, #000000, transparent)',
            }}
          />
          <div
            className="sona-color-picker-thumb"
            style={{
              position: 'absolute',
              left: `${hsv.s}%`,
              top: `${100 - hsv.v}%`,
              width: '14px',
              height: '14px',
              borderRadius: '50%',
              border: '2px solid #ffffff',
              boxShadow: '0 0 0 1px rgba(0, 0, 0, 0.25), 0 2px 4px rgba(0, 0, 0, 0.25)',
              transform: 'translate(-50%, -50%)',
              pointerEvents: 'none',
            }}
          />
        </div>

        {/* 2. Middle Row: Eyedropper, Color Swatch, Hue Rainbow Slider */}
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
          <button
            type="button"
            className="btn btn-icon sona-color-picker-btn"
            onMouseDown={(e) => e.stopPropagation()}
            onClick={handleEyeDropper}
            data-tooltip="Pick color"
            data-tooltip-pos="top"
            aria-label="Pick color"
            style={{
              width: '26px',
              height: '26px',
              padding: 0,
              borderRadius: '6px',
              border: '1px solid var(--color-border)',
              background: 'var(--color-bg-secondary)',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              color: 'var(--color-text-secondary)',
              flexShrink: 0,
            }}
          >
            <Pipette size={13} />
          </button>
          <div
            style={{
              width: '26px',
              height: '26px',
              borderRadius: '50%',
              backgroundColor: currentHex,
              border: '1px solid var(--color-border)',
              flexShrink: 0,
              boxShadow: 'inset 0 0 0 1px rgba(0,0,0,0.08)',
            }}
          />
          {/* Hue Slider with Rounded Corners & Theme Gray Border */}
          <div
            ref={hueRef}
            className="sona-color-picker-hue"
            onPointerDown={handleHuePointerDown}
            style={{
              position: 'relative',
              flex: 1,
              height: '14px',
              borderRadius: '7px',
              background:
                'linear-gradient(to right, #ff0000 0%, #ffff00 17%, #00ff00 33%, #00ffff 50%, #0000ff 67%, #ff00ff 83%, #ff0000 100%)',
              cursor: 'pointer',
              border: '1px solid var(--color-border)',
              boxSizing: 'border-box',
              touchAction: 'none',
            }}
          >
            <div
              style={{
                position: 'absolute',
                left: `${(hsv.h / 360) * 100}%`,
                top: '50%',
                width: '16px',
                height: '16px',
                borderRadius: '50%',
                border: '2px solid #ffffff',
                backgroundColor: `hsl(${hsv.h}, 100%, 50%)`,
                boxShadow: '0 0 0 1px rgba(0, 0, 0, 0.25), 0 1px 3px rgba(0, 0, 0, 0.3)',
                transform: 'translate(-50%, -50%)',
                pointerEvents: 'none',
              }}
            />
          </div>
        </div>

        {/* 3. Bottom Row: RGB / HEX Input with Rounded Corners & Theme Gray Borders */}
        <div style={{ display: 'flex', alignItems: 'flex-start', gap: '6px' }}>
          {format === 'rgb' ? (
            <div style={{ display: 'flex', gap: '6px', flex: 1 }}>
              {(['r', 'g', 'b'] as const).map((channel) => (
                <div
                  key={channel}
                  style={{
                    display: 'flex',
                    flexDirection: 'column',
                    alignItems: 'center',
                    flex: 1,
                    gap: '2px',
                  }}
                >
                  <input
                    type="number"
                    min={0}
                    max={255}
                    value={rgb[channel]}
                    onMouseDown={(e) => e.stopPropagation()}
                    onChange={(e) => handleRgbChange(channel, e.target.value)}
                    style={{
                      width: '100%',
                      height: '28px',
                      textAlign: 'center',
                      borderRadius: '6px',
                      border: '1px solid var(--color-border)',
                      background: 'var(--color-bg-input)',
                      color: 'var(--color-text-primary)',
                      fontSize: '0.8125rem',
                      padding: '0 2px',
                      boxSizing: 'border-box',
                      outline: 'none',
                    }}
                  />
                  <span
                    style={{
                      fontSize: '0.6875rem',
                      color: 'var(--color-text-muted)',
                      textTransform: 'uppercase',
                      fontWeight: 600,
                    }}
                  >
                    {channel}
                  </span>
                </div>
              ))}
            </div>
          ) : (
            <div
              style={{
                display: 'flex',
                flexDirection: 'column',
                alignItems: 'center',
                flex: 1,
                gap: '2px',
              }}
            >
              <input
                type="text"
                value={hexInput}
                onMouseDown={(e) => e.stopPropagation()}
                onChange={(e) => handleHexInputChange(e.target.value)}
                className="sona-color-picker-input"
                style={{
                  width: '100%',
                  height: '28px',
                  textAlign: 'center',
                  borderRadius: '6px',
                  border: '1px solid var(--color-border)',
                  background: 'var(--color-bg-input)',
                  color: 'var(--color-text-primary)',
                  fontFamily: 'var(--font-mono)',
                  fontSize: '0.8125rem',
                  padding: '0 6px',
                  boxSizing: 'border-box',
                  outline: 'none',
                }}
              />
              <span
                style={{
                  fontSize: '0.6875rem',
                  color: 'var(--color-text-muted)',
                  textTransform: 'uppercase',
                  fontWeight: 600,
                }}
              >
                HEX
              </span>
            </div>
          )}
          <button
            type="button"
            className="btn btn-icon sona-color-picker-btn"
            onMouseDown={(e) => e.stopPropagation()}
            onClick={() => setFormat((prev) => (prev === 'rgb' ? 'hex' : 'rgb'))}
            data-tooltip="Toggle RGB / HEX"
            data-tooltip-pos="top"
            aria-label="Toggle RGB / HEX"
            style={{
              width: '28px',
              height: '28px',
              padding: 0,
              borderRadius: '6px',
              border: '1px solid var(--color-border)',
              background: 'var(--color-bg-secondary)',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              color: 'var(--color-text-muted)',
              flexShrink: 0,
            }}
          >
            <ChevronsUpDown size={14} />
          </button>
        </div>
        {/* Hidden fallback color input */}
        <input
          ref={fallbackInputRef}
          type="color"
          style={{
            position: 'absolute',
            opacity: 0,
            pointerEvents: 'none',
            width: 0,
            height: 0,
            border: 0,
            padding: 0,
          }}
          value={hexInput.startsWith('#') && hexInput.length === 7 ? hexInput : '#6366F1'}
          onChange={(e) => {
            const val = e.target.value.toUpperCase();
            const nextRgb = hexToRgb(val);
            setRgb(nextRgb);
            setHsv(rgbToHsv(nextRgb.r, nextRgb.g, nextRgb.b));
            setHexInput(val);
            onChange(val);
          }}
          tabIndex={-1}
          aria-hidden="true"
        />
      </div>
    </ModalPortal>
  );
}
