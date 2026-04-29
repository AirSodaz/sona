import { execFileSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const eslintBin = fileURLToPath(new URL('../node_modules/eslint/bin/eslint.js', import.meta.url));
const LINTABLE_EXTENSIONS = new Set(['.cjs', '.js', '.jsx', '.mjs', '.ts', '.tsx']);

function getStagedFiles() {
  const output = execFileSync(
    'git',
    ['diff', '--cached', '--name-only', '--diff-filter=ACMR', '-z'],
    { encoding: 'utf8' }
  );
  return output.split('\u0000').filter(Boolean);
}

const args = process.argv.slice(2);
const filesToCheck = args.length > 0 ? args : getStagedFiles();

if (filesToCheck.length === 0) {
  process.exit(0);
}

const conflictMarkerPattern = /^(<{7}|={7}|>{7})( .*)?$/m;

const errors = [];
const lintTargets = new Set();

for (const fileArg of filesToCheck) {
  const repoRelativePath = path.isAbsolute(fileArg) ? path.relative(process.cwd(), fileArg) : fileArg;
  const relativePath = repoRelativePath.split(path.sep).join('/');
  const extension = path.extname(relativePath).toLowerCase();

  if (LINTABLE_EXTENSIONS.has(extension)) {
    lintTargets.add(relativePath);
  }

  let stagedContent;
  try {
    stagedContent = execFileSync('git', ['show', `:${relativePath}`], { encoding: 'utf8' });
  } catch {
    continue;
  }

  if (stagedContent.includes('\u0000')) {
    continue;
  }

  if (conflictMarkerPattern.test(stagedContent)) {
    errors.push(`${relativePath}: staged content contains unresolved merge conflict markers.`);
  }

  if (extension === '.json') {
    try {
      JSON.parse(stagedContent);
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Unknown JSON parse error';
      errors.push(`${relativePath}: staged JSON is invalid (${message}).`);
    }
  }
}

if (errors.length > 0) {
  console.error('Staged file checks failed:\n');
  for (const error of errors) {
    console.error(`- ${error}`);
  }
  process.exit(1);
}

if (lintTargets.size === 0) {
  process.exit(0);
}

try {
  execFileSync(
    process.execPath,
    [eslintBin, '--max-warnings=0', '--no-warn-ignored', ...Array.from(lintTargets)],
    { stdio: 'inherit' }
  );
} catch {
  process.exit(1);
}
