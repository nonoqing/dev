import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';

import {
  auditMarkdownLinks,
  extractDestinations,
  isGovernedMarkdown,
} from './check-doc-links.mjs';

function createFixture(files) {
  const root = mkdtempSync(path.join(tmpdir(), 'bitfun-doc-links-'));
  for (const [file, content] of Object.entries(files)) {
    const target = path.join(root, file);
    mkdirSync(path.dirname(target), { recursive: true });
    writeFileSync(target, content);
  }
  return root;
}

function audit(files) {
  const root = createFixture(files);
  const result = auditMarkdownLinks({
    repoRoot: root,
    repositoryFiles: Object.keys(files),
  });
  return { result, root };
}

test('accepts exact files, directories, and Unicode Markdown anchors', (t) => {
  const { result, root } = audit({
    'docs/README.md':
      '# Index\n\n[Guide](guide.md#中文-section)\n\n[Folder](topic/)\n',
    'docs/guide.md': '# Guide\n\n## 中文 Section\n',
    'docs/topic/README.md': '# Topic\n',
  });
  t.after(() => rmSync(root, { recursive: true, force: true }));

  assert.deepEqual(result.issues, []);
});

test('reports missing files and missing Markdown anchors separately', (t) => {
  const { result, root } = audit({
    'docs/README.md':
      '# Index\n\n[Missing](missing.md)\n\n[Bad anchor](guide.md#absent)\n',
    'docs/guide.md': '# Guide\n\n## Present\n',
  });
  t.after(() => rmSync(root, { recursive: true, force: true }));

  assert.equal(result.issues.length, 2);
  assert.match(result.issues[0].message, /target does not exist/);
  assert.match(result.issues[1].message, /anchor does not exist/);
});

test('rejects path casing that would fail on a case-sensitive checkout', (t) => {
  const { result, root } = audit({
    'docs/README.md': '# Index\n\n[Guide](Guide.md)\n',
    'docs/guide.md': '# Guide\n',
  });
  t.after(() => rmSync(root, { recursive: true, force: true }));

  assert.equal(result.issues.length, 2);
  assert.match(result.issues[0].message, /exact tracked casing/);
  assert.match(result.issues[1].message, /no inbound link/);
});

test('ignores Markdown-looking examples in fenced and inline code', () => {
  const tick = String.fromCharCode(96);
  const content =
    '~~~md\n[not a link](missing.md)\n~~~\n' +
    tick +
    '[also not a link](missing.md)' +
    tick +
    '\n[real](guide.md)\n';

  assert.deepEqual(extractDestinations(content), [
    { destination: 'guide.md', line: 5 },
  ]);
});

test('extracts reference-definition targets outside fences', () => {
  assert.deepEqual(extractDestinations('[guide]: <topic/guide.md#section>\n'), [
    { destination: 'topic/guide.md#section', line: 1 },
  ]);
});

test('limits the check to governed documentation surfaces', () => {
  assert.equal(isGovernedMarkdown('docs/specs/example.md'), true);
  assert.equal(isGovernedMarkdown('src/web-ui/AGENTS.md'), true);
  assert.equal(isGovernedMarkdown('src/example/README.zh-CN.md'), true);
  assert.equal(isGovernedMarkdown('src/example/prompt.md'), false);
  assert.equal(isGovernedMarkdown('docs/example.local.txt'), false);
});

test('reports governed docs that are absent from every index or route', (t) => {
  const { result, root } = audit({
    'docs/README.md': '# Index\n',
    'docs/orphan.md': '# Orphan\n',
  });
  t.after(() => rmSync(root, { recursive: true, force: true }));

  assert.equal(result.issues.length, 1);
  assert.match(result.issues[0].message, /no inbound link/);
});
