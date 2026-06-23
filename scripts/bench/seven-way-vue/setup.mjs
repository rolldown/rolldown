#!/usr/bin/env node
// Sparse-clone Elk and snapshot the list of .vue files into corpus.json.

import { execSync } from 'node:child_process';
import { existsSync, mkdirSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const FIXTURE_DIR = resolve(__dirname, '.fixture');
const REPO_DIR = join(FIXTURE_DIR, 'elk');
const APP_DIR = join(REPO_DIR, 'app');
const CORPUS_JSON = join(__dirname, 'corpus.json');

// Pinned for reproducible benches. Update when refreshing the corpus.
const ELK_REV = '0b92391bef794433bee9d590074b4b166802ade4';

mkdirSync(FIXTURE_DIR, { recursive: true });

if (!existsSync(REPO_DIR)) {
  console.log('Cloning Elk (sparse, depth=1)...');
  execSync(
    `git clone --depth=1 --filter=blob:none --sparse https://github.com/elk-zone/elk "${REPO_DIR}"`,
    { stdio: 'inherit' },
  );
  execSync(`git -C "${REPO_DIR}" fetch --depth=1 origin ${ELK_REV}`, { stdio: 'inherit' });
  execSync(`git -C "${REPO_DIR}" checkout ${ELK_REV}`, { stdio: 'inherit' });
  execSync(`git -C "${REPO_DIR}" sparse-checkout set app`, { stdio: 'inherit' });
} else {
  console.log('Reusing existing clone at', REPO_DIR);
}

if (!existsSync(APP_DIR)) {
  throw new Error(`Expected ${APP_DIR} to exist after sparse-checkout`);
}

const files = [];
function walk(dir) {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    const s = statSync(p);
    if (s.isDirectory()) walk(p);
    else if (s.isFile() && name.endsWith('.vue')) {
      files.push(relative(APP_DIR, p));
    }
  }
}
walk(APP_DIR);

files.sort();
writeFileSync(
  CORPUS_JSON,
  JSON.stringify({ root: APP_DIR, rev: ELK_REV, files }, null, 2) + '\n',
);
console.log(`Wrote ${files.length} .vue files to ${relative(process.cwd(), CORPUS_JSON)}`);
