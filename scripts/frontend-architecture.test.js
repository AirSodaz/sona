import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const frontendRoot = path.join(repoRoot, 'platforms', 'desktop', 'frontend', 'src');

function toPosix(value) {
  return value.replaceAll(path.sep, '/');
}

function relativeToFrontend(filePath) {
  return toPosix(path.relative(frontendRoot, filePath));
}

function isProductionModule(filePath) {
  const relativePath = relativeToFrontend(filePath);
  return /\.(?:ts|tsx)$/u.test(relativePath)
    && !relativePath.includes('/__tests__/')
    && !relativePath.startsWith('__tests__/')
    && !/\.test\.(?:ts|tsx)$/u.test(relativePath);
}

function productionModules() {
  const modules = [];

  function visit(directory) {
    for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
      const entryPath = path.join(directory, entry.name);
      if (entry.isDirectory()) {
        if (entry.name !== '__tests__') {
          visit(entryPath);
        }
      } else if (isProductionModule(entryPath)) {
        modules.push(path.normalize(entryPath));
      }
    }
  }

  visit(frontendRoot);
  return modules.sort();
}

function moduleSpecifiers(source) {
  const specifiers = [];
  const patterns = [
    /\b(?:import|export)\s+(?:type\s+)?(?:[^'";]*?\s+from\s+)?['"]([^'"]+)['"]/gu,
    /\bimport\s*\(\s*['"]([^'"]+)['"]\s*\)/gu,
  ];

  for (const pattern of patterns) {
    for (const match of source.matchAll(pattern)) {
      specifiers.push(match[1]);
    }
  }

  return [...new Set(specifiers)];
}

function resolveLocalModule(sourceFile, specifier, moduleSet) {
  if (!specifier.startsWith('.')) {
    return null;
  }

  const basePath = path.resolve(path.dirname(sourceFile), specifier);
  const candidates = [
    basePath,
    `${basePath}.ts`,
    `${basePath}.tsx`,
    path.join(basePath, 'index.ts'),
    path.join(basePath, 'index.tsx'),
  ];

  return candidates
    .map((candidate) => path.normalize(candidate))
    .find((candidate) => moduleSet.has(candidate)) ?? null;
}

function buildDependencyGraph() {
  const modules = productionModules();
  const moduleSet = new Set(modules);
  const graph = new Map();
  const externalImports = new Map();

  for (const modulePath of modules) {
    const source = fs.readFileSync(modulePath, 'utf8');
    const specifiers = moduleSpecifiers(source);
    graph.set(
      modulePath,
      specifiers
        .map((specifier) => resolveLocalModule(modulePath, specifier, moduleSet))
        .filter((target) => target !== null),
    );
    externalImports.set(
      modulePath,
      specifiers.filter((specifier) => !specifier.startsWith('.')),
    );
  }

  return { externalImports, graph };
}

function stronglyConnectedComponents(graph) {
  let nextIndex = 0;
  const stack = [];
  const onStack = new Set();
  const indexes = new Map();
  const lowLinks = new Map();
  const components = [];

  function visit(modulePath) {
    indexes.set(modulePath, nextIndex);
    lowLinks.set(modulePath, nextIndex);
    nextIndex += 1;
    stack.push(modulePath);
    onStack.add(modulePath);

    for (const dependency of graph.get(modulePath) ?? []) {
      if (!indexes.has(dependency)) {
        visit(dependency);
        lowLinks.set(
          modulePath,
          Math.min(lowLinks.get(modulePath), lowLinks.get(dependency)),
        );
      } else if (onStack.has(dependency)) {
        lowLinks.set(
          modulePath,
          Math.min(lowLinks.get(modulePath), indexes.get(dependency)),
        );
      }
    }

    if (lowLinks.get(modulePath) !== indexes.get(modulePath)) {
      return;
    }

    const component = [];
    let current;
    do {
      current = stack.pop();
      onStack.delete(current);
      component.push(current);
    } while (current !== modulePath);

    if (component.length > 1) {
      components.push(component);
    }
  }

  for (const modulePath of graph.keys()) {
    if (!indexes.has(modulePath)) {
      visit(modulePath);
    }
  }

  return components;
}

function isWithin(relativePath, directory) {
  return relativePath === directory || relativePath.startsWith(`${directory}/`);
}

test('desktop frontend production modules have no dependency cycles', () => {
  const { graph } = buildDependencyGraph();
  const cycles = stronglyConnectedComponents(graph).map((component) => (
    component.map(relativeToFrontend).sort()
  ));

  assert.deepEqual(
    cycles,
    [],
    'move shared DTOs and pure helpers down instead of allowing a circular import',
  );
});

test('Tauri packages stay behind the frontend platform boundary', () => {
  const { externalImports } = buildDependencyGraph();
  const violations = [];

  for (const [modulePath, imports] of externalImports) {
    const source = relativeToFrontend(modulePath);
    for (const specifier of imports.filter((value) => value.startsWith('@tauri-apps/'))) {
      const insidePlatform = isWithin(source, 'services/tauri/platform');
      const isInvokeBoundary = source === 'services/tauri/invoke.ts';
      const allowed = specifier === '@tauri-apps/api/core'
        ? insidePlatform || isInvokeBoundary
        : insidePlatform;
      if (!allowed) {
        violations.push(`${source} imports ${specifier}`);
      }
    }
  }

  assert.deepEqual(
    violations,
    [],
    'all static and dynamic @tauri-apps imports belong in services/tauri/platform ' +
      '(except api/core in services/tauri/invoke.ts)',
  );
});

test('desktop frontend layers do not import upward', () => {
  const { graph } = buildDependencyGraph();
  const violations = [];

  for (const [modulePath, dependencies] of graph) {
    const source = relativeToFrontend(modulePath);
    for (const dependency of dependencies) {
      const target = relativeToFrontend(dependency);

      if (
        (isWithin(source, 'types') || isWithin(source, 'constants'))
        && ['services', 'stores', 'hooks', 'components'].some((layer) => isWithin(target, layer))
      ) {
        violations.push(`${source} -> ${target} (stable contract imports an upper layer)`);
      }

      if (
        isWithin(source, 'services/tauri')
        && (
          ['stores', 'hooks', 'components'].some((layer) => isWithin(target, layer))
          || (isWithin(target, 'services') && !isWithin(target, 'services/tauri'))
        )
      ) {
        violations.push(`${source} -> ${target} (Tauri boundary imports application/UI code)`);
      }

      if (
        isWithin(source, 'services')
        && !isWithin(source, 'services/tauri')
        && ['hooks', 'components'].some((layer) => isWithin(target, layer))
      ) {
        violations.push(`${source} -> ${target} (service imports UI code)`);
      }

      if (
        isWithin(source, 'stores')
        && ['hooks', 'components'].some((layer) => isWithin(target, layer))
      ) {
        violations.push(`${source} -> ${target} (store imports UI code)`);
      }
    }
  }

  assert.deepEqual(violations, []);
});

test('Tauri command contracts depend only on stable DTO owners', () => {
  const { graph } = buildDependencyGraph();
  const contractsPath = path.normalize(
    path.join(frontendRoot, 'services', 'tauri', 'contracts.ts'),
  );
  const allowedFiles = new Set([
    'bindings.ts',
    'services/tauri/commands.ts',
  ]);
  const violations = (graph.get(contractsPath) ?? [])
    .map(relativeToFrontend)
    .filter((target) => !isWithin(target, 'types') && !allowedFiles.has(target));

  assert.deepEqual(
    violations,
    [],
    'Tauri contracts may use generated bindings, command names, and src/types DTOs only',
  );
});

test('desktop frontend dependency rules are published in both architecture guides', () => {
  for (const guide of ['architecture.md', 'architecture.zh-CN.md']) {
    const source = fs.readFileSync(path.join(repoRoot, 'docs', guide), 'utf8');
    assert.match(source, /<a id="desktop-frontend-dependencies"><\/a>/u, guide);
    assert.match(source, /scripts\/frontend-architecture\.test\.js/u, guide);
    assert.match(source, /services\/tauri\/platform/u, guide);
  }
});
