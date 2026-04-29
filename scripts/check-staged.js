import { execFileSync } from 'node:child_process';
import path from 'node:path';

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

for (const fileArg of filesToCheck) {
  const repoRelativePath = path.isAbsolute(fileArg) ? path.relative(process.cwd(), fileArg) : fileArg;
  const relativePath = repoRelativePath.split(path.sep).join('/');
  const extension = path.extname(relativePath).toLowerCase();

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
