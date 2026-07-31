import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const read = (relativePath) =>
  fs.readFileSync(path.join(repoRoot, relativePath), 'utf8');

const script = read('scripts/harbor-build-musl-container.sh');
const dockerfile = read('scripts/harbor-build-musl/Dockerfile');
const packageJson = JSON.parse(read('package.json'));

assert.match(
  script,
  /cargo build --locked --release -p bitfun-cli --target \$\{TARGET_TRIPLE\} --bins/,
);
assert.match(script, /PRIMARY="\$\{RELEASE_DIR\}\/bitfun"/);
assert.match(script, /LEGACY="\$\{RELEASE_DIR\}\/bitfun-cli"/);
assert.match(script, /target":"\/usr\/local\/bin\/bitfun"/);
assert.match(script, /target":"\/usr\/local\/bin\/bitfun-cli"/);
assert.match(script, /ubuntu:22\.04/);
assert.match(script, /debian:bookworm-slim/);
assert.match(script, /alpine:3\.20/);
assert.match(script, /readelf -l/);

assert.match(dockerfile, /musl-tools/);
assert.match(dockerfile, /rustup target add x86_64-unknown-linux-musl/);

assert.equal(
  packageJson.scripts['harbor:cli:musl'],
  'scripts/harbor-build-musl-container.sh compile-and-test',
);
assert.equal(
  packageJson.scripts['harbor:cli:musl:build'],
  'scripts/harbor-build-musl-container.sh compile',
);
assert.equal(
  packageJson.scripts['harbor:cli:musl:test'],
  'scripts/harbor-build-musl-container.sh test-binaries',
);
