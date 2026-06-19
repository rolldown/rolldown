#!/usr/bin/env node
// Sparse-clone Infisical and snapshot the list of frontend source files into corpus.json.

import { execSync } from 'node:child_process';
import { existsSync, mkdirSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const FIXTURE_DIR = resolve(__dirname, '.fixture');
const REPO_DIR = join(FIXTURE_DIR, 'infisical');
const FRONTEND_DIR = join(REPO_DIR, 'frontend');
const CORPUS_JSON = join(__dirname, 'corpus.json');

mkdirSync(FIXTURE_DIR, { recursive: true });

if (!existsSync(REPO_DIR)) {
  console.log('Cloning Infisical (sparse, depth=1)...');
  execSync(
    `git clone --depth=1 --filter=blob:none --sparse https://github.com/Infisical/infisical "${REPO_DIR}"`,
    { stdio: 'inherit' },
  );
  execSync(`git -C "${REPO_DIR}" sparse-checkout set frontend`, { stdio: 'inherit' });
} else {
  console.log('Reusing existing clone at', REPO_DIR);
}

if (!existsSync(FRONTEND_DIR)) {
  throw new Error(`Expected ${FRONTEND_DIR} to exist after sparse-checkout`);
}

const EXT = new Set(['.tsx', '.ts', '.jsx', '.js']);
const SKIP_DIRS = new Set(['node_modules', '.next', 'dist', 'build', '.git']);

const files = [];
function walk(dir) {
  for (const name of readdirSync(dir)) {
    if (SKIP_DIRS.has(name)) continue;
    const p = join(dir, name);
    const s = statSync(p);
    if (s.isDirectory()) walk(p);
    else if (s.isFile()) {
      // Skip TypeScript declaration files — they describe types, not runnable code.
      if (/\.d\.[cm]?ts$/.test(name)) continue;
      const dot = name.lastIndexOf('.');
      if (dot > 0 && EXT.has(name.slice(dot))) {
        files.push(relative(FRONTEND_DIR, p));
      }
    }
  }
}
walk(FRONTEND_DIR);

files.sort();
writeFileSync(
  CORPUS_JSON,
  JSON.stringify({ root: FRONTEND_DIR, files }, null, 2) + '\n',
);
console.log(`Wrote ${files.length} files to ${relative(process.cwd(), CORPUS_JSON)}`);
