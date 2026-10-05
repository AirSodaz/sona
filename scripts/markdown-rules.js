import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

/**
 * Lightweight, zero-dependency Markdown validation rules for Sona.
 * Catches common formatting mistakes, unclosed fences, syntax defects, and stale CLI terms.
 */

const FENCE_PATTERN = /^([ \t]*)(`{3,}|~{3,})(.*)$/u;
const HEADING_DEFECT_PATTERN = /^([ \t]*)#{1,6}[^# \t\r\n]/u;
const EMPTY_LINK_PATTERN = /\[[^\]]*\]\(\s*\)/u;

export function validateMarkdown(content, relativePath = '') {
  const errors = [];
  const lines = content.split(/\r?\n/u);

  let inCodeBlock = false;
  let codeBlockStartLine = 0;
  let codeBlockFenceChar = '';
  let codeBlockFenceLen = 0;

  for (let i = 0; i < lines.length; i++) {
    const lineNum = i + 1;
    const line = lines[i];

    const fenceMatch = line.match(FENCE_PATTERN);
    if (fenceMatch) {
      const fenceMarker = fenceMatch[2];
      const fenceChar = fenceMarker[0];
      const fenceLen = fenceMarker.length;
      const infoString = fenceMatch[3].trim();

      if (!inCodeBlock) {
        inCodeBlock = true;
        codeBlockStartLine = lineNum;
        codeBlockFenceChar = fenceChar;
        codeBlockFenceLen = fenceLen;

        if (infoString.length === 0) {
          errors.push(
            `Line ${lineNum}: fenced code block is missing a language identifier (e.g. \`\`\`bash, \`\`\`json, \`\`\`text).`
          );
        }
      } else {
        // Closing fence: same character, at least as long as opening fence
        if (fenceChar === codeBlockFenceChar && fenceLen >= codeBlockFenceLen) {
          inCodeBlock = false;
        }
      }
      continue;
    }

    // Skip syntax checks inside code blocks
    if (inCodeBlock) {
      continue;
    }

    // Check heading formatting: # must be followed by a space
    if (HEADING_DEFECT_PATTERN.test(line)) {
      errors.push(
        `Line ${lineNum}: Markdown heading is missing a space after '#' (e.g. '# Title', not '#Title').`
      );
    }

    // Check empty links: [text]()
    if (EMPTY_LINK_PATTERN.test(line)) {
      errors.push(`Line ${lineNum}: found empty Markdown link target '[]()'.`);
    }

    // Specific contract check for CLI guide
    if (relativePath.replace(/\\/gu, '/').endsWith('docs/cli.md')) {
      if (/\bexport\s+transcript\b/iu.test(line)) {
        errors.push(
          `Line ${lineNum}: found stale command 'export transcript'. Use direct 'export' instead.`
        );
      }
      if (/--model-id\b/u.test(line) && !/alias\s+--model-id/iu.test(line) && !/`--model-id`/u.test(line)) {
        errors.push(
          `Line ${lineNum}: found bare '--model-id'. Use unified '-m / --model' (or explicitly note alias).`
        );
      }
    }
  }

  if (inCodeBlock) {
    errors.push(
      `Line ${codeBlockStartLine}: unclosed fenced code block at end of file.`
    );
  }

  return errors;
}

// CLI execution support: `node scripts/markdown-rules.js [files...]`
if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
  const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
  const targetFiles = process.argv.slice(2);
  let filesToCheck = targetFiles;

  if (filesToCheck.length === 0) {
    filesToCheck = [];
    const docsDir = path.join(repoRoot, 'docs');
    if (fs.existsSync(docsDir)) {
      for (const entry of fs.readdirSync(docsDir)) {
        if (entry.endsWith('.md')) {
          filesToCheck.push(path.join(docsDir, entry));
        }
      }
    }
    for (const entry of fs.readdirSync(repoRoot)) {
      if (entry.endsWith('.md')) {
        filesToCheck.push(path.join(repoRoot, entry));
      }
    }
  }

  let totalErrors = 0;
  for (const filePath of filesToCheck) {
    const rel = path.relative(repoRoot, filePath);
    const content = fs.readFileSync(filePath, 'utf8');
    const errors = validateMarkdown(content, rel);
    if (errors.length > 0) {
      console.error(`\nMarkdown validation failed for ${rel}:`);
      for (const err of errors) {
        console.error(`  - ${err}`);
      }
      totalErrors += errors.length;
    }
  }

  if (totalErrors > 0) {
    console.error(`\nTotal ${totalErrors} Markdown formatting error(s) found.`);
    process.exit(1);
  } else {
    console.log(`All ${filesToCheck.length} Markdown file(s) passed formatting rules.`);
  }
}
