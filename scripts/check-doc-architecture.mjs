#!/usr/bin/env node

import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { extractDestinations } from './check-doc-links.mjs';

const defaultRepoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

export const PRODUCT_ARCHITECTURE_AUTHORITY =
  'docs/architecture/product-architecture.md';

export const REQUIRED_LEVEL_ZERO_HEADINGS = [
  '### 2.1 Logical View · Level 0',
  '### 2.2 Development View · Level 0',
  '### 2.3 Process View · Level 0',
  '### 2.4 Physical View · Level 0',
  '### 2.5 Scenarios (+1) · Level 0',
];

const REQUIRED_DRILL_DOWN_TARGETS = [
  'agent-runtime-deployment-design.md',
  'extensions/plugin-runtime-design.md',
  'app-server-architecture.md',
  'remote-workspace-transport.md',
];

const REQUIRED_METHOD_MARKERS = ['Kruchten', 'C4', 'arc42'];

function lineNumber(content, index) {
  return content.slice(0, Math.max(index, 0)).split(/\r?\n/).length;
}

function localTarget(destination) {
  const withoutFragment = destination.split('#', 1)[0].split('?', 1)[0];
  if (!withoutFragment || /^(?:[a-z][a-z0-9+.-]*:|\/\/|\/)/i.test(withoutFragment)) {
    return null;
  }
  return path.posix.normalize(withoutFragment.replace(/\\/g, '/'));
}

export function auditProductArchitecture(content) {
  const issues = [];
  const authorityHeading = '## 2. 4+1 Architecture Views';
  const authorityStart = content.indexOf(authorityHeading);
  const nextSection =
    authorityStart >= 0 ? content.indexOf('\n## 3.', authorityStart) : -1;

  if (authorityStart < 0) {
    return [
      {
        line: 1,
        message: 'missing product-level 4+1 authority heading',
      },
    ];
  }

  const authorityEnd = nextSection >= 0 ? nextSection : content.length;
  const section = content.slice(authorityStart, authorityEnd);

  for (const marker of REQUIRED_METHOD_MARKERS) {
    if (!section.includes(marker)) {
      issues.push({
        line: lineNumber(content, authorityStart),
        message: `4+1 authority is missing method marker: ${marker}`,
      });
    }
  }

  const positions = REQUIRED_LEVEL_ZERO_HEADINGS.map((heading) => ({
    heading,
    index: section.indexOf(heading),
  }));

  for (const { heading, index } of positions) {
    if (index < 0) {
      issues.push({
        line: lineNumber(content, authorityStart),
        message: `missing required Level 0 view: ${heading}`,
      });
    }
  }

  const presentPositions = positions.filter(({ index }) => index >= 0);
  for (let index = 1; index < presentPositions.length; index += 1) {
    if (presentPositions[index].index < presentPositions[index - 1].index) {
      issues.push({
        line: lineNumber(content, authorityStart + presentPositions[index].index),
        message: 'Level 0 views are not in Logical/Development/Process/Physical/Scenarios order',
      });
      break;
    }
  }

  for (const [index, current] of positions.entries()) {
    if (current.index < 0) {
      continue;
    }
    const later = positions
      .slice(index + 1)
      .find((candidate) => candidate.index > current.index);
    const viewEnd = later ? later.index : section.length;
    const view = section.slice(current.index, viewEnd);
    if (!view.includes('```mermaid')) {
      issues.push({
        line: lineNumber(content, authorityStart + current.index),
        message: `${current.heading} has no Mermaid view`,
      });
    }
  }

  const destinations = new Set(
    extractDestinations(section)
      .map(({ destination }) => localTarget(destination))
      .filter(Boolean),
  );
  for (const target of REQUIRED_DRILL_DOWN_TARGETS) {
    if (!destinations.has(target)) {
      issues.push({
        line: lineNumber(content, authorityStart),
        message: `4+1 authority is missing Level 1 drill-down: ${target}`,
      });
    }
  }

  return issues;
}

export function auditArchitectureAuthority(repoRoot) {
  const authorityPath = path.join(repoRoot, PRODUCT_ARCHITECTURE_AUTHORITY);
  if (!existsSync(authorityPath)) {
    return [
      {
        line: 1,
        message: `architecture authority does not exist: ${PRODUCT_ARCHITECTURE_AUTHORITY}`,
      },
    ];
  }
  return auditProductArchitecture(readFileSync(authorityPath, 'utf8'));
}

function runCli() {
  const repoRoot = process.env.BITFUN_DOC_ARCHITECTURE_CHECK_ROOT
    ? path.resolve(process.env.BITFUN_DOC_ARCHITECTURE_CHECK_ROOT)
    : defaultRepoRoot;
  const issues = auditArchitectureAuthority(repoRoot);

  if (issues.length > 0) {
    console.error('Documentation architecture check failed:');
    for (const issue of issues) {
      console.error(
        `${PRODUCT_ARCHITECTURE_AUTHORITY}:${issue.line} (${issue.message})`,
      );
    }
    process.exitCode = 1;
    return;
  }

  console.log('Documentation architecture check passed (product-level 4+1 authority).');
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  runCli();
}
