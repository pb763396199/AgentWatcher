import { copyFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const prototypePath = resolve(repoRoot, 'agentwatcher-windows-overflow-prototype.html');
const runtimePath = resolve(repoRoot, 'ui', 'index.html');

copyFileSync(prototypePath, runtimePath);