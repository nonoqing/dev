import { execFileSync } from 'node:child_process';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const sourceDir = path.join(root, 'source');
const distDir = path.join(root, 'dist');
const requiredSourceFiles = ['index.html', 'style.css', 'ui.js', 'worker.js', 'esm_dependencies.json'];
const installedMeta = JSON.parse(await readFile(path.join(root, 'meta.json'), 'utf8'));
const outputPath = path.join(distDir, `cat-personality-test-v${installedMeta.version}.bfminiapp`);
const packageMeta = {
  name: installedMeta.name,
  description: installedMeta.description,
  icon: installedMeta.icon,
  category: installedMeta.category,
  tags: installedMeta.tags,
  version: installedMeta.version,
  permissions: installedMeta.permissions,
  i18n: installedMeta.i18n,
};

await mkdir(distDir, { recursive: true });
await rm(outputPath, { force: true });
const stagingDir = await mkdtemp(path.join(tmpdir(), 'cat-personality-market-'));

try {
  await mkdir(path.join(stagingDir, 'source'));
  await writeFile(path.join(stagingDir, 'meta.json'), `${JSON.stringify(packageMeta, null, 2)}\n`);
  for (const filename of requiredSourceFiles) {
    const content = await readFile(path.join(sourceDir, filename));
    await writeFile(path.join(stagingDir, 'source', filename), content);
  }
  execFileSync('zip', [
    '-X',
    '-q',
    outputPath,
    'meta.json',
    ...requiredSourceFiles.map((filename) => `source/${filename}`),
  ], { cwd: stagingDir, stdio: 'inherit' });
} finally {
  await rm(stagingDir, { recursive: true, force: true });
}

console.log(outputPath);
