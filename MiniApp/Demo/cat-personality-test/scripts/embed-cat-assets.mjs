import { readFile, readdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const sourceDir = path.join(root, 'source');
const assetsDir = path.join(sourceDir, 'assets');
const avatarPattern = /^cat-\d{2}-[a-z0-9-]+\.webp$/;

const avatarFiles = (await readdir(assetsDir)).filter((name) => avatarPattern.test(name)).sort();
if (avatarFiles.length !== 12) {
  throw new Error(`Expected 12 source portraits, found ${avatarFiles.length}`);
}

const avatarData = {};
for (const fileName of avatarFiles) {
  const id = fileName.replace(/\.webp$/, '');
  const inputPath = path.join(assetsDir, fileName);
  const webp = await readFile(inputPath);
  avatarData[id] = `data:image/webp;base64,${webp.toString('base64')}`;
}

const backgroundInput = path.join(assetsDir, 'paper-patchwork-bg.webp');
const backgroundWebp = await readFile(backgroundInput);
const backgroundData = `data:image/webp;base64,${backgroundWebp.toString('base64')}`;

const paperTextureInput = path.join(assetsDir, 'paper-ivory-texture.webp');
const paperTextureWebp = await readFile(paperTextureInput);
const paperTextureData = `data:image/webp;base64,${paperTextureWebp.toString('base64')}`;

const pawStampInput = path.join(assetsDir, 'paw-stamp.webp');
const pawStampWebp = await readFile(pawStampInput);
const pawStampData = `data:image/webp;base64,${pawStampWebp.toString('base64')}`;

const uiPath = path.join(sourceDir, 'ui.js');
const uiSource = await readFile(uiPath, 'utf8');
const avatarBlock = `/* CAT_AVATAR_DATA_START */ ${JSON.stringify(avatarData, null, 2)} /* CAT_AVATAR_DATA_END */`;
const avatarMarker = /\/\* CAT_AVATAR_DATA_START \*\/[\s\S]*?\/\* CAT_AVATAR_DATA_END \*\//;
if (!avatarMarker.test(uiSource)) throw new Error('Avatar data marker was not found in source/ui.js');
const nextUiSource = uiSource.replace(avatarMarker, avatarBlock);
await writeFile(uiPath, nextUiSource);

const cssPath = path.join(sourceDir, 'style.css');
const cssSource = await readFile(cssPath, 'utf8');
const backgroundToken = /--cat-background-image:\s*url\("[^\"]*"\);/;
if (!backgroundToken.test(cssSource)) throw new Error('Background image token was not found in source/style.css');
const paperTextureToken = /--cat-paper-texture:\s*(?:none|url\("[^\"]*"\));/;
if (!paperTextureToken.test(cssSource)) throw new Error('Paper texture token was not found in source/style.css');
const pawStampToken = /--cat-paw-stamp:\s*(?:none|url\("[^\"]*"\));/;
if (!pawStampToken.test(cssSource)) throw new Error('Paw stamp token was not found in source/style.css');
const nextCssSource = cssSource
  .replace(backgroundToken, `--cat-background-image: url("${backgroundData}");`)
  .replace(paperTextureToken, `--cat-paper-texture: url("${paperTextureData}");`)
  .replace(pawStampToken, `--cat-paw-stamp: url("${pawStampData}");`);
await writeFile(cssPath, nextCssSource);

console.log(`Embedded ${avatarFiles.length} portraits, one background texture, one paper texture, and one paw stamp.`);
