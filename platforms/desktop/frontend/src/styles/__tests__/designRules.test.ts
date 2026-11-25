import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const srcDir = path.resolve(__dirname, '../..');

function walkFiles(dir: string, exts: RegExp): string[] {
  const results: string[] = [];
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      if (entry.name === '__tests__' || entry.name === 'node_modules') {
        continue;
      }
      results.push(...walkFiles(fullPath, exts));
    } else if (exts.test(entry.name)) {
      results.push(fullPath);
    }
  }
  return results;
}

const ALLOWED_TITLE_PROP_COMPONENTS = new Set([
  'Modal',
  'PanelModal',
  'SettingsItem',
  'SettingsSection',
  'SettingsPageHeader',
  'RailItemContent',
  'ToolHeroHeader',
  'EmptyState',
  'ConfirmDialog',
  'SectionCard',
  'NotificationSection',
  'QueueGroup',
  'TrendCard',
  'UsageBreakdown',
  'SettingsAccordion',
  'SettingsLocationCard',
  'FeatureCard',
  'TranscriptWorkbench',
  'StorageCategoryCard',
  'QuickStartCard',
  'UsageMetric',
]);

describe('Desktop Design System Rules Guardrails', () => {
  it('forbids legacy data-tooltip attributes in source files', () => {
    const sourceFiles = walkFiles(srcDir, /\.(ts|tsx|css)$/);
    const violations: string[] = [];

    for (const file of sourceFiles) {
      const content = fs.readFileSync(file, 'utf-8');
      const lines = content.split('\n');
      for (let i = 0; i < lines.length; i++) {
        if (lines[i].includes('data-tooltip')) {
          violations.push(`${path.relative(srcDir, file)}:${i + 1}`);
        }
      }
    }

    expect(violations).toEqual([]);
  });

  it('forbids native HTML title attributes on intrinsic elements and Button/IconButton primitives', () => {
    const tsxFiles = walkFiles(srcDir, /\.tsx$/);
    const violations: string[] = [];

    for (const file of tsxFiles) {
      const lines = fs.readFileSync(file, 'utf-8').split('\n');
      for (let i = 0; i < lines.length; i++) {
        const line = lines[i];
        const inlineMatch = line.match(
          /<([a-z][a-zA-Z0-9]*|Button|IconButton|SplitButton)\b[^>]*\btitle=/
        );
        if (inlineMatch) {
          violations.push(`${path.relative(srcDir, file)}:${i + 1} (<${inlineMatch[1]}>)`);
          continue;
        }
        if (/^\s*title=/.test(line)) {
          // Walk back to the enclosing JSX opening tag, ignoring nested icon props like icon={<Foo />}
          let tag = '';
          let braceDepth = 0;
          for (let j = i - 1; j >= Math.max(0, i - 25); j--) {
            const prev = lines[j];
            for (let k = prev.length - 1; k >= 0; k--) {
              if (prev[k] === '}') braceDepth++;
              else if (prev[k] === '{') braceDepth--;
            }
            if (braceDepth <= 0) {
              const tagMatch = prev.match(/^\s*<([a-zA-Z0-9_]+)(?:\s|$)/);
              if (tagMatch) {
                tag = tagMatch[1];
                break;
              }
            }
          }
          if (tag && !ALLOWED_TITLE_PROP_COMPONENTS.has(tag)) {
            violations.push(`${path.relative(srcDir, file)}:${i + 1} (<${tag}>)`);
          }
        }
      }
    }

    expect(violations).toEqual([]);
  });

  it('forbids non-primary solid black backgrounds and legacy blue/purple fallbacks in CSS', () => {
    const cssFiles = walkFiles(srcDir, /\.css$/).filter(
      (file) => path.basename(file) !== 'tokens.css'
    );
    const forbiddenColorRe =
      /(#646cff|#0a84ff|#007aff|#6366f1|#60a5fa|#4a9eff|#2563eb|#2b7fff|--color-accent-blue|background(?:-color)?:\s*var\(--color-(?:accent-primary|primary)(?:,|\)))/i;

    const violations: string[] = [];
    for (const file of cssFiles) {
      const lines = fs.readFileSync(file, 'utf-8').split('\n');
      for (let i = 0; i < lines.length; i++) {
        if (forbiddenColorRe.test(lines[i])) {
          violations.push(`${path.relative(srcDir, file)}:${i + 1}: ${lines[i].trim()}`);
        }
      }
    }

    expect(violations).toEqual([]);
  });
});
