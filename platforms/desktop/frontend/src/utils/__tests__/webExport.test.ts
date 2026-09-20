import { describe, expect, it } from 'vitest';
import type { TranscriptSegment } from '../../types/transcript';
import {
  exportToJson,
  exportToMarkdown,
  exportToSrt,
  exportToTxt,
  exportToVtt,
} from '../webExport';

const mockSegments: TranscriptSegment[] = [
  {
    id: 'seg-1',
    start: 0,
    end: 2.5,
    text: 'Hello world',
    speaker: { id: 'spk-1', label: 'Speaker 1', kind: 'identified' },
    isFinal: true,
  },
  {
    id: 'seg-2',
    start: 2.5,
    end: 5.8,
    text: 'Welcome to Sona',
    translation: '欢迎使用 Sona',
    isFinal: true,
  },
];

describe('webExport', () => {
  it('exports segments to SRT format', () => {
    const srt = exportToSrt(mockSegments);
    expect(srt).toContain('1\n00:00:00,000 --> 00:00:02,500\n[Speaker 1] Hello world');
    expect(srt).toContain('2\n00:00:02,500 --> 00:00:05,800\nWelcome to Sona');
  });

  it('exports segments to SRT bilingual mode', () => {
    const srt = exportToSrt(mockSegments, 'bilingual');
    expect(srt).toContain('Welcome to Sona\n欢迎使用 Sona');
  });

  it('exports segments to VTT format', () => {
    const vtt = exportToVtt(mockSegments);
    expect(vtt.startsWith('WEBVTT\n\n')).toBe(true);
    expect(vtt).toContain('00:00:00.000 --> 00:00:02.500\n<v Speaker 1>Hello world');
  });

  it('exports segments to plain text format', () => {
    const txt = exportToTxt(mockSegments);
    expect(txt).toBe('Speaker 1: Hello world\n\nWelcome to Sona');
  });

  it('exports segments to Markdown format', () => {
    const md = exportToMarkdown(mockSegments);
    expect(md).toContain('`00:00:00` **Speaker 1** Hello world');
  });

  it('exports segments to JSON format', () => {
    const json = exportToJson(mockSegments);
    const parsed = JSON.parse(json);
    expect(parsed.length).toBe(2);
    expect(parsed[0].text).toBe('Hello world');
  });
});
