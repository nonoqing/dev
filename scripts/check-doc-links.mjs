#!/usr/bin/env node

import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const defaultRepoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

function normalizePath(file) {
  return file.replace(/\\/g, '/').replace(/^\.\//, '');
}

function gitFiles(repoRoot, args) {
  try {
    return execFileSync('git', args, { cwd: repoRoot, encoding: 'utf8' })
      .split(/\r?\n/)
      .filter(Boolean)
      .map(normalizePath);
  } catch {
    return [];
  }
}

export function isGovernedMarkdown(file) {
  const normalized = normalizePath(file);
  const basename = path.posix.basename(normalized);

  if (!normalized.toLowerCase().endsWith('.md')) {
    return false;
  }

  return (
    normalized.startsWith('docs/') ||
    /^(?:README(?:[._-].+)?|AGENTS(?:[._-].+)?|CONTRIBUTING(?:[._-].+)?|SECURITY)\.md$/i.test(
      normalized,
    ) ||
    /^(?:README(?:[._-].+)?|AGENTS(?:[._-].+)?|LOGGING)\.md$/i.test(basename)
  );
}

function stripInlineCode(line) {
  return line.replace(/\x60+[^\x60]*\x60+/g, '');
}

function normalizeDestination(raw) {
  const value = raw.trim();
  if (!value) {
    return null;
  }

  if (value.startsWith('<')) {
    const closing = value.indexOf('>');
    return closing > 0 ? value.slice(1, closing) : null;
  }

  const match = value.match(/^(?:\\.|[^\s])+/);
  return match ? match[0].replace(/\\([ ()])/g, '$1') : null;
}

export function extractDestinations(content) {
  const destinations = [];
  const lines = content.split(/\r?\n/);
  let fence = null;

  for (const [index, originalLine] of lines.entries()) {
    const fenceMatch = originalLine.match(/^\s{0,3}((?:\x60{3,}|~{3,}))/);
    if (fenceMatch) {
      const marker = fenceMatch[1];
      if (!fence) {
        fence = { character: marker[0], length: marker.length };
      } else if (marker[0] === fence.character && marker.length >= fence.length) {
        fence = null;
      }
      continue;
    }

    if (fence) {
      continue;
    }

    const line = stripInlineCode(originalLine);
    const reference = line.match(/^\s{0,3}\[[^\]]+\]:\s*(.+)$/);
    if (reference) {
      const destination = normalizeDestination(reference[1]);
      if (destination) {
        destinations.push({ destination, line: index + 1 });
      }
    }

    let cursor = 0;
    while (cursor < line.length) {
      const opening = line.indexOf('](', cursor);
      if (opening < 0) {
        break;
      }

      let depth = 0;
      let escaped = false;
      let closing = -1;
      for (let position = opening + 2; position < line.length; position += 1) {
        const character = line[position];
        if (escaped) {
          escaped = false;
          continue;
        }
        if (character === '\\') {
          escaped = true;
          continue;
        }
        if (character === '(') {
          depth += 1;
          continue;
        }
        if (character === ')' && depth > 0) {
          depth -= 1;
          continue;
        }
        if (character === ')') {
          closing = position;
          break;
        }
      }

      if (closing < 0) {
        break;
      }

      const destination = normalizeDestination(line.slice(opening + 2, closing));
      if (destination) {
        destinations.push({ destination, line: index + 1 });
      }
      cursor = closing + 1;
    }
  }

  return destinations;
}

function decodeLinkPart(value) {
  try {
    return decodeURIComponent(value);
  } catch {
    return value;
  }
}

function headingSlug(text) {
  return text
    .replace(/\s+#+\s*$/, '')
    .replace(/!\[([^\]]*)\]\([^)]*\)/g, '$1')
    .replace(/\[([^\]]+)\]\([^)]*\)/g, '$1')
    .replace(/<[^>]+>/g, '')
    .replace(/\x60+/g, '')
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s_-]/gu, '')
    .replace(/\s/g, '-');
}

function collectAnchors(content) {
  const anchors = new Set();
  const duplicates = new Map();
  let fence = null;

  for (const line of content.split(/\r?\n/)) {
    const fenceMatch = line.match(/^\s{0,3}((?:\x60{3,}|~{3,}))/);
    if (fenceMatch) {
      const marker = fenceMatch[1];
      if (!fence) {
        fence = { character: marker[0], length: marker.length };
      } else if (marker[0] === fence.character && marker.length >= fence.length) {
        fence = null;
      }
      continue;
    }
    if (fence) {
      continue;
    }

    for (const match of line.matchAll(/\b(?:id|name)=["']([^"']+)["']/gi)) {
      anchors.add(match[1]);
    }

    const heading = line.match(/^\s{0,3}#{1,6}\s+(.+?)\s*$/);
    if (!heading) {
      continue;
    }

    const base = headingSlug(heading[1]);
    if (!base) {
      continue;
    }
    const duplicate = duplicates.get(base) ?? 0;
    anchors.add(duplicate === 0 ? base : base + '-' + duplicate);
    duplicates.set(base, duplicate + 1);
  }

  return anchors;
}

function splitDestination(destination) {
  const hashIndex = destination.indexOf('#');
  const beforeHash = hashIndex >= 0 ? destination.slice(0, hashIndex) : destination;
  const fragment = hashIndex >= 0 ? decodeLinkPart(destination.slice(hashIndex + 1)) : '';
  const queryIndex = beforeHash.indexOf('?');
  const file = queryIndex >= 0 ? beforeHash.slice(0, queryIndex) : beforeHash;
  return { file: decodeLinkPart(file), fragment };
}

function isExternalDestination(destination) {
  return (
    /^(?:[a-z][a-z0-9+.-]*:|\/\/)/i.test(destination) ||
    destination.startsWith('/')
  );
}

export function auditMarkdownLinks({ repoRoot, repositoryFiles }) {
  const normalizedFiles = [...new Set(repositoryFiles.map(normalizePath))];
  const repositoryFileSet = new Set(normalizedFiles);
  const governedFiles = normalizedFiles.filter(isGovernedMarkdown);
  const anchorsByFile = new Map();
  const inboundDocs = new Set();
  const issues = [];

  function hasDirectory(directory) {
    const prefix = directory.replace(/\/+$/, '') + '/';
    return normalizedFiles.some((file) => file.startsWith(prefix));
  }

  function anchorsFor(file) {
    if (!anchorsByFile.has(file)) {
      anchorsByFile.set(
        file,
        collectAnchors(readFileSync(path.join(repoRoot, file), 'utf8')),
      );
    }
    return anchorsByFile.get(file);
  }

  for (const source of governedFiles) {
    const sourcePath = path.join(repoRoot, source);
    if (!existsSync(sourcePath)) {
      continue;
    }

    const content = readFileSync(sourcePath, 'utf8');
    for (const { destination, line } of extractDestinations(content)) {
      if (isExternalDestination(destination)) {
        continue;
      }

      const { file, fragment } = splitDestination(destination);
      const target = file
        ? normalizePath(path.posix.normalize(path.posix.join(path.posix.dirname(source), file)))
        : source;

      if (target === '..' || target.startsWith('../')) {
        issues.push({
          source,
          line,
          destination,
          message: 'escapes the repository root',
        });
        continue;
      }

      const targetIsFile = repositoryFileSet.has(target);
      const targetIsDirectory = hasDirectory(target);
      if (!targetIsFile && !targetIsDirectory) {
        issues.push({
          source,
          line,
          destination,
          message: 'target does not exist with exact tracked casing',
        });
        continue;
      }

      if (
        targetIsFile &&
        target !== source &&
        target.startsWith('docs/') &&
        target.toLowerCase().endsWith('.md')
      ) {
        inboundDocs.add(target);
      }
      if (targetIsDirectory) {
        const directoryIndex = target.replace(/\/+$/, '') + '/README.md';
        if (directoryIndex !== source && repositoryFileSet.has(directoryIndex)) {
          inboundDocs.add(directoryIndex);
        }
      }

      if (
        fragment &&
        targetIsFile &&
        target.toLowerCase().endsWith('.md') &&
        !/^L\d+(?:-L\d+)?$/i.test(fragment) &&
        !anchorsFor(target).has(fragment.replace(/^user-content-/, ''))
      ) {
        issues.push({
          source,
          line,
          destination,
          message: 'Markdown anchor does not exist',
        });
      }
    }
  }

  for (const file of governedFiles) {
    if (
      file.startsWith('docs/') &&
      file !== 'docs/README.md' &&
      !inboundDocs.has(file)
    ) {
      issues.push({
        source: file,
        line: 1,
        destination: file,
        message: 'document has no inbound link from the governed documentation graph',
      });
    }
  }

  return { checkedFiles: governedFiles.length, issues };
}

export function collectRepositoryFiles(repoRoot) {
  const candidates = [
    ...gitFiles(repoRoot, ['ls-files', '--cached']),
    ...gitFiles(repoRoot, ['ls-files', '--others', '--exclude-standard']),
  ];

  return [...new Set(candidates)].filter((file) =>
    existsSync(path.join(repoRoot, file)),
  );
}

function runCli() {
  const repoRoot = process.env.BITFUN_DOC_LINK_CHECK_ROOT
    ? path.resolve(process.env.BITFUN_DOC_LINK_CHECK_ROOT)
    : defaultRepoRoot;
  const result = auditMarkdownLinks({
    repoRoot,
    repositoryFiles: collectRepositoryFiles(repoRoot),
  });

  if (result.issues.length > 0) {
    console.error('Documentation link check failed:');
    for (const issue of result.issues) {
      console.error(
        issue.source +
          ':' +
          issue.line +
          ' -> ' +
          issue.destination +
          ' (' +
          issue.message +
          ')',
      );
    }
    process.exitCode = 1;
    return;
  }

  console.log(
    'Documentation link check passed (' + result.checkedFiles + ' files scanned).',
  );
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  runCli();
}
