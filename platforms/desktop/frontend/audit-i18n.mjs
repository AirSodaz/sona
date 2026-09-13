// One-off audit: find t('...') keys used in source but missing from en.json baseline.
// Dynamic template-literal keys (t(`a.${x}`)) are reported separately.
import fs from 'node:fs';
import path from 'node:path';

const srcDir = path.resolve('src');
const files = [];
function walk(dir) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    if (e.isDirectory()) {
      if (e.name === 'node_modules' || e.name === 'dist') continue;
      walk(path.join(dir, e.name));
    } else if (/\.(tsx?|jsx?)$/.test(e.name) && !/\.test\.(tsx?|jsx?)$/.test(e.name)) {
      files.push(path.join(dir, e.name));
    }
  }
}
walk(srcDir);

const staticRe = /\bt\(\s*'([a-zA-Z0-9_.]+)'/g;
const dynamicRe = /\bt\(\s*`([^`]*\$\{[^`]*)`/g;
const usedKeys = new Map(); // key -> [files]
const dynamicPatterns = [];

for (const f of files) {
  const content = fs.readFileSync(f, 'utf8');
  let m;
  while ((m = staticRe.exec(content))) {
    if (!usedKeys.has(m[1])) usedKeys.set(m[1], []);
    usedKeys.get(m[1]).push(path.relative(srcDir, f));
  }
  while ((m = dynamicRe.exec(content))) {
    dynamicPatterns.push({ file: path.relative(srcDir, f), pattern: m[1] });
  }
}

const en = JSON.parse(fs.readFileSync(path.join(srcDir, 'locales/en.json'), 'utf8'));
function flatten(obj, prefix = '') {
  return Object.entries(obj).flatMap(([k, v]) =>
    v && typeof v === 'object' && !Array.isArray(v)
      ? flatten(v, prefix ? `${prefix}.${k}` : k)
      : [prefix ? `${prefix}.${k}` : k]
  );
}
const enKeys = new Set(flatten(en));

// i18next plural/context suffixes count as a match for the base key.
const suffixes = ['_zero', '_one', '_two', '_few', '_many', '_other'];
const isCovered = (key) => enKeys.has(key) || suffixes.some((s) => enKeys.has(`${key}${s}`));

const missing = [...usedKeys.entries()]
  .filter(([key]) => !isCovered(key))
  .map(([key, refs]) => ({ key, refs: [...new Set(refs)] }))
  .sort((a, b) => a.key.localeCompare(b.key));

console.info(`Scanned ${files.length} source files, ${usedKeys.size} static keys.`);
console.info(`\n=== Missing from en.json (${missing.length}) ===`);
for (const { key, refs } of missing) {
  console.info(`${key}\n    used in: ${refs.slice(0, 3).join(', ')}`);
}
console.info(`\n=== Dynamic key patterns (${dynamicPatterns.length}) ===`);
for (const { file, pattern } of dynamicPatterns) {
  console.info(`${pattern}   [${file}]`);
}
