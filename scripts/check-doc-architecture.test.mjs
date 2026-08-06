import assert from 'node:assert/strict';
import test from 'node:test';

import { auditProductArchitecture } from './check-doc-architecture.mjs';

const viewNames = [
  'Logical View · Level 0',
  'Development View · Level 0',
  'Process View · Level 0',
  'Physical View · Level 0',
  'Scenarios (+1) · Level 0',
];

function validAuthority() {
  const views = viewNames
    .map(
      (name, index) =>
        `### 2.${index + 1} ${name}\n\n\`\`\`mermaid\nflowchart LR\n  A --> B\n\`\`\`\n`,
    )
    .join('\n');
  return `# Product architecture

## 2. 4+1 Architecture Views

Kruchten, C4, and arc42 define the view method. Scenarios validate the first four views.

[Runtime](agent-runtime-deployment-design.md)
[Plugins](extensions/plugin-runtime-design.md)
[App Server](app-server-architecture.md)
[Remote](remote-workspace-transport.md)

${views}
## 3. Interfaces
`;
}

test('accepts the complete product-level 4+1 authority', () => {
  assert.deepEqual(auditProductArchitecture(validAuthority()), []);
});

test('reports a missing Level 0 view', () => {
  const content = validAuthority().replace(
    /### 2\.3 Process View · Level 0[\s\S]*?(?=### 2\.4)/,
    '',
  );
  const issues = auditProductArchitecture(content);

  assert.ok(issues.some(({ message }) => message.includes('Process View')));
});

test('reports a Level 0 view without a Mermaid diagram', () => {
  const content = validAuthority().replace(
    '### 2.4 Physical View · Level 0\n\n```mermaid\nflowchart LR\n  A --> B\n```',
    '### 2.4 Physical View · Level 0\n\nPhysical deployment text.',
  );
  const issues = auditProductArchitecture(content);

  assert.ok(
    issues.some(({ message }) =>
      message.includes('Physical View · Level 0 has no Mermaid view'),
    ),
  );
});

test('reports Level 0 views in the wrong order', () => {
  const content = validAuthority()
    .replace('### 2.1 Logical View · Level 0', '### swap')
    .replace(
      '### 2.2 Development View · Level 0',
      '### 2.1 Logical View · Level 0',
    )
    .replace('### swap', '### 2.2 Development View · Level 0');
  const issues = auditProductArchitecture(content);

  assert.ok(issues.some(({ message }) => message.includes('not in Logical')));
});

test('reports a missing Level 1 drill-down', () => {
  const content = validAuthority().replace(
    '[Remote](remote-workspace-transport.md)\n',
    '',
  );
  const issues = auditProductArchitecture(content);

  assert.ok(
    issues.some(({ message }) =>
      message.includes('remote-workspace-transport.md'),
    ),
  );
});

test('reports a missing architecture method marker', () => {
  const content = validAuthority().replace('Kruchten, C4, and arc42', 'The view method');
  const issues = auditProductArchitecture(content);

  assert.equal(
    issues.filter(({ message }) => message.includes('method marker')).length,
    3,
  );
});
