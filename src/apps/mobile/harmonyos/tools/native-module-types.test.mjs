import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';

const harmonyRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '..',
);
const typePackageDir = path.join(
  harmonyRoot,
  'entry',
  'src',
  'main',
  'cpp',
  'types',
  'libbitfun_crypto',
);
const packagePath = path.join(typePackageDir, 'oh-package.json5');
const declarationPath = path.join(typePackageDir, 'Index.d.ts');
const entryPackagePath = path.join(harmonyRoot, 'entry', 'oh-package.json5');

test('bitfun crypto native module publishes ArkTS declarations', () => {
  assert.ok(
    existsSync(packagePath),
    'libbitfun_crypto.so must publish an oh-package.json5 type mapping',
  );
  assert.ok(
    existsSync(declarationPath),
    'libbitfun_crypto.so must publish an Index.d.ts declaration',
  );

  const metadata = JSON.parse(readFileSync(packagePath, 'utf8'));
  assert.equal(metadata.name, 'libbitfun_crypto.so');
  assert.equal(metadata.types, './Index.d.ts');

  const entryMetadata = JSON.parse(readFileSync(entryPackagePath, 'utf8'));
  assert.equal(
    entryMetadata.dependencies?.['libbitfun_crypto.so'],
    'file:./src/main/cpp/types/libbitfun_crypto',
    'entry must map libbitfun_crypto.so to its local ArkTS declarations',
  );

  const declaration = readFileSync(declarationPath, 'utf8').replace(/\s+/g, ' ');
  assert.match(
    declaration,
    /export const argon2idRaw:\s*\(password: Uint8Array, salt: Uint8Array, memory: number, time: number, lanes: number\) => Uint8Array;/,
  );
});
