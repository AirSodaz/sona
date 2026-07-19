import assert from 'node:assert/strict';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';
import {
  CURRENT_PUBLIC_STRING_ERROR_DEBT,
  excludeReviewedApiServerStringErrors,
  findPublicStringErrors,
  scanPublicStringErrorsInDirectory,
} from './architecture-policy.mjs';

const fixtureDirectory = path.join(
  path.dirname(fileURLToPath(import.meta.url)),
  'test-fixtures',
  'rust-public-errors',
);

test('reviewed non-host Rust public APIs do not return string errors', () => {
  assert.deepEqual(
    findPublicStringErrors(),
    [...CURRENT_PUBLIC_STRING_ERROR_DEBT.values()],
  );
});

test('scanner recognizes required public signatures and excludes test-only code', () => {
  assert.deepEqual(
    scanPublicStringErrorsInDirectory({
      packageName: 'fixture',
      sourceDirectory: fixtureDirectory,
      relativeTo: fixtureDirectory,
    }),
    [
      { package: 'fixture', file: 'lib.rs', symbol: 'production_error', signature: 'pub fn' },
      { package: 'fixture', file: 'public_api.rs', symbol: 'multiline', signature: 'pub fn' },
      { package: 'fixture', file: 'public_api.rs', symbol: 'async_multiline', signature: 'pub async fn' },
      { package: 'fixture', file: 'public_api.rs', symbol: 'on_event', signature: 'pub trait method' },
      { package: 'fixture', file: 'public_api.rs', symbol: 'CallbackResult', signature: 'pub type' },
    ],
  );
});

test('exact API Server handlers are exempted, and only through their reviewed contract', () => {
  const reviewed = [
    'handle_info',
    'handle_job_status',
    'handle_transcribe',
  ].map((symbol) => ({
    package: 'sona-api-server',
    file: 'adapters/api_server/src/lib.rs',
    symbol,
    signature: 'pub async fn',
  }));
  const nearMisses = [
    { ...reviewed[0], signature: 'pub fn' },
    { ...reviewed[1], file: 'adapters/api_server/src/other.rs' },
    { ...reviewed[2], package: 'fixture-api-server' },
  ];

  assert.deepEqual(
    excludeReviewedApiServerStringErrors([...reviewed, ...nearMisses]),
    nearMisses,
  );
  assert.deepEqual(
    findPublicStringErrors().filter(({ symbol }) =>
      ['handle_info', 'handle_job_status', 'handle_transcribe'].includes(symbol)),
    [],
  );
});
