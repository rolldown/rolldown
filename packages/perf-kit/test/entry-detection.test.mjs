// Entry detection must read the built index.html as the browser does. An
// attribute value can have double, single or no quotes (HTML minifiers drop the
// quotes), and a script tag in a comment, in script text, in <noscript> or in a
// <template> does not load.

import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

const HARNESS = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', 'harness.mjs');

/**
 * The entry that `scan` detects in a build of these files. The build has no
 * sourcemap, so scan stops right after detection with an error that names the
 * entry. Other output comes back whole, so a failed assertion shows it.
 */
function detectedEntry(files) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'perf-kit-entry-'));
  try {
    const dist = path.join(root, 'dist');
    for (const [file, text] of Object.entries(files)) {
      fs.mkdirSync(path.dirname(path.join(dist, file)), { recursive: true });
      fs.writeFileSync(path.join(dist, file), text);
    }
    const run = spawnSync(process.execPath, [HARNESS, 'scan', '--dist', dist], {
      encoding: 'utf8',
      env: { ...process.env, ROLLDOWN_PERF_KIT_STATE: path.join(root, 'state') },
      timeout: 20_000,
    });
    const output = run.stdout + run.stderr;
    return /entry (\S+) has no sourcemap/.exec(output)?.[1] ?? output;
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

const NAV = 'console.log("nav");';

for (const src of ['"/a/nav.js"', "'/a/nav.js'", '/a/nav.js']) {
  test(`detects the entry from src=${src}`, () => {
    const html = `<!doctype html><html><head><script defer src=${src}></script></head><body>hi</body></html>`;
    assert.equal(detectedEntry({ 'index.html': html, 'a/nav.js': NAV }), 'a/nav.js');
  });
}

for (const type of ['"module"', "'module'", 'module']) {
  test(`a module script with type=${type} wins over a bigger classic script`, () => {
    // When no script is a module, the biggest local script wins: legacy.js is bigger.
    const html = `<!doctype html><script src="/a/legacy.js"></script><script type=${type} src="/a/nav.js"></script>`;
    const files = { 'index.html': html, 'a/legacy.js': ' '.repeat(4000), 'a/nav.js': NAV };
    assert.equal(detectedEntry(files), 'a/nav.js');
  });
}

for (const [where, decoy] of [
  ['a comment', '<!-- <script type="module" src="/a/old.js"></script> -->'],
  ['script text', `<script>const tag = '<script type="module" src="/a/old.js">';</script>`],
  ['<noscript>', '<noscript><script type="module" src="/a/old.js"></script></noscript>'],
  ['<template>', '<template><script type="module" src="/a/old.js"></script></template>'],
]) {
  test(`a module script tag in ${where} is not the entry`, () => {
    const html = `<!doctype html>${decoy}<script type="module" src="/a/nav.js"></script>`;
    assert.equal(detectedEntry({ 'index.html': html, 'a/nav.js': NAV }), 'a/nav.js');
  });
}

for (const [what, tag] of [
  ['no space between attributes', '<script type="module"src="/a/nav.js"></script>'],
  [
    'a > in a quoted value before src',
    '<script data-note="a>b" type="module" src="/a/nav.js"></script>',
  ],
]) {
  test(`detects the entry with ${what}`, () => {
    assert.equal(
      detectedEntry({ 'index.html': `<!doctype html>${tag}`, 'a/nav.js': NAV }),
      'a/nav.js',
    );
  });
}

test('detects an Astro island chunk from a bare component-url', () => {
  const html =
    '<!doctype html><astro-island uid=x component-url=/a/nav.js component-export=default></astro-island>';
  assert.equal(detectedEntry({ 'index.html': html, 'a/nav.js': NAV }), 'a/nav.js');
});
