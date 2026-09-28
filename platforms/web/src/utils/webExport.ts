import type { TranscriptSegment } from '../types/transcript';

function formatTimestamp(seconds: number, separator: string): string {
  const safeSeconds = Math.max(0, seconds);
  const totalMs = Math.round(safeSeconds * 1000);
  const hours = Math.floor(totalMs / 3600000);
  const minutes = Math.floor((totalMs % 3600000) / 60000);
  const secs = Math.floor((totalMs % 60000) / 1000);
  const millis = totalMs % 1000;

  const hh = hours.toString().padStart(2, '0');
  const mm = minutes.toString().padStart(2, '0');
  const ss = secs.toString().padStart(2, '0');
  const mmm = millis.toString().padStart(3, '0');

  return `${hh}:${mm}:${ss}${separator}${mmm}`;
}

export function exportToSrt(
  segments: TranscriptSegment[],
  mode: 'original' | 'translation' | 'bilingual' = 'original'
): string {
  return segments
    .filter((s) => s.text?.trim())
    .map((s, idx) => {
      const startTime = formatTimestamp(s.start, ',');
      const endTime = formatTimestamp(s.end, ',');
      let text = s.text.trim();
      if (mode === 'translation' && s.translation) {
        text = s.translation.trim();
      } else if (mode === 'bilingual' && s.translation) {
        text = `${s.text.trim()}\n${s.translation.trim()}`;
      }
      if (s.speaker?.label) {
        text = `[${s.speaker.label}] ${text}`;
      }
      return `${idx + 1}\n${startTime} --> ${endTime}\n${text}\n`;
    })
    .join('\n');
}

export function exportToVtt(
  segments: TranscriptSegment[],
  mode: 'original' | 'translation' | 'bilingual' = 'original'
): string {
  const body = segments
    .filter((s) => s.text?.trim())
    .map((s) => {
      const startTime = formatTimestamp(s.start, '.');
      const endTime = formatTimestamp(s.end, '.');
      let text = s.text.trim();
      if (mode === 'translation' && s.translation) {
        text = s.translation.trim();
      } else if (mode === 'bilingual' && s.translation) {
        text = `${s.text.trim()}\n${s.translation.trim()}`;
      }
      if (s.speaker?.label) {
        text = `<v ${s.speaker.label}>${text}`;
      }
      return `${startTime} --> ${endTime}\n${text}\n`;
    })
    .join('\n');
  return `WEBVTT\n\n${body}`;
}

export function exportToTxt(
  segments: TranscriptSegment[],
  mode: 'original' | 'translation' | 'bilingual' = 'original'
): string {
  return segments
    .filter((s) => s.text?.trim())
    .map((s) => {
      let text = s.text.trim();
      if (mode === 'translation' && s.translation) {
        text = s.translation.trim();
      } else if (mode === 'bilingual' && s.translation) {
        text = `${s.text.trim()}\n${s.translation.trim()}`;
      }
      if (s.speaker?.label) {
        return `${s.speaker.label}: ${text}`;
      }
      return text;
    })
    .join('\n\n');
}

export function exportToMarkdown(
  segments: TranscriptSegment[],
  mode: 'original' | 'translation' | 'bilingual' = 'original'
): string {
  return segments
    .filter((s) => s.text?.trim())
    .map((s) => {
      const time = formatTimestamp(s.start, ':').slice(0, 8);
      let text = s.text.trim();
      if (mode === 'translation' && s.translation) {
        text = s.translation.trim();
      } else if (mode === 'bilingual' && s.translation) {
        text = `${s.text.trim()}\n\n> ${s.translation.trim()}`;
      }
      const speaker = s.speaker?.label ? `**${s.speaker.label}** ` : '';
      return `\`${time}\` ${speaker}${text}`;
    })
    .join('\n\n');
}

export function exportToJson(segments: TranscriptSegment[]): string {
  return JSON.stringify(segments, null, 2);
}

export function downloadFile(content: string, filename: string, mimeType: string): void {
  const blob = new Blob([content], { type: mimeType });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}
