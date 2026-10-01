import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

globalThis.fixtureLog = [];
globalThis.fixtureInitializationCount = 0;

const entry = await import('./dist/entry.js');
assert.deepEqual(globalThis.fixtureLog, []);

const lightA = await entry.loadLightA();
assert.equal(lightA.value, 'light-a-marker');
assert.deepEqual(globalThis.fixtureLog, ['initialize', 'consumer-light-a:light-a-marker']);

const lightB = await entry.loadLightB();
assert.equal(lightB.value, 'light-b-marker');
assert.deepEqual(globalThis.fixtureLog, [
  'initialize',
  'consumer-light-a:light-a-marker',
  'consumer-light-b:light-b-marker',
]);

const heavy = await entry.loadHeavy();
assert.match(heavy.value, /^heavy-marker:heavy-payload-01:/);
assert.deepEqual(globalThis.fixtureLog, [
  'initialize',
  'consumer-light-a:light-a-marker',
  'consumer-light-b:light-b-marker',
  `consumer-heavy:${heavy.value}`,
]);
assert.equal(globalThis.fixtureInitializationCount, 1);

const distDir = fileURLToPath(new URL('./dist/', import.meta.url));

function listJavaScriptFiles(directory, relativeDirectory = '') {
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const relativePath = path.join(relativeDirectory, entry.name);
    return entry.isDirectory()
      ? listJavaScriptFiles(path.join(directory, entry.name), relativePath)
      : entry.name.endsWith('.js')
        ? [relativePath]
        : [];
  });
}

const jsFiles = listJavaScriptFiles(distDir);
const output = new Map(
  jsFiles.map((file) => [file, fs.readFileSync(path.join(distDir, file), 'utf8')]),
);

function findOutputContaining(marker) {
  const matches = [...output].filter(([, code]) => code.includes(marker));
  assert.equal(
    matches.length,
    1,
    `expected one output containing ${marker}; got ${matches.map(([file]) => file).join(', ')}`,
  );
  return matches[0][0];
}

function staticClosure(rootFile) {
  const closure = new Set();
  const pending = [rootFile];
  const staticSpecifier = /\b(?:import|export)\s*(?:[^"'`;]*?\sfrom\s*)?["'](\.[^"']+)["']/g;

  while (pending.length > 0) {
    const file = pending.pop();
    if (closure.has(file)) continue;
    closure.add(file);

    const code = output.get(file);
    assert.ok(code, `missing emitted output ${file}`);
    for (const match of code.matchAll(staticSpecifier)) {
      const targetUrl = new URL(match[1], pathToFileURL(path.join(distDir, file)));
      const target = path.relative(distDir, fileURLToPath(targetUrl));
      if (output.has(target) && !closure.has(target)) pending.push(target);
    }
  }

  return {
    files: closure,
    code: [...closure].map((file) => output.get(file)).join('\n'),
  };
}

const entryClosure = staticClosure(findOutputContaining('loadLightA'));
const lightAClosure = staticClosure(findOutputContaining('consumer-light-a:'));
const lightBClosure = staticClosure(findOutputContaining('consumer-light-b:'));
const heavyClosure = staticClosure(findOutputContaining('consumer-heavy:'));
const reachableFiles = new Set([
  ...entryClosure.files,
  ...lightAClosure.files,
  ...lightBClosure.files,
  ...heavyClosure.files,
]);
assert.deepEqual(
  [...output.keys()].filter((file) => !reachableFiles.has(file)),
  [],
  'every emitted chunk should be reachable from an entry or dynamic entry',
);

assert.match(lightAClosure.code, /light-a-marker/);
assert.match(lightAClosure.code, /fixtureInitializationCount/);
assert.match(lightBClosure.code, /light-b-marker/);
assert.match(lightBClosure.code, /fixtureInitializationCount/);
assert.match(heavyClosure.code, /heavy-marker/);
assert.match(heavyClosure.code, /fixtureInitializationCount/);

if (globalThis.__configName !== 'preserve-modules') {
  assert.doesNotMatch(lightAClosure.code, /light-b-marker/);
  assert.doesNotMatch(lightAClosure.code, /heavy-marker/);
  assert.doesNotMatch(lightBClosure.code, /light-a-marker/);
  assert.doesNotMatch(lightBClosure.code, /heavy-marker/);
  assert.doesNotMatch(heavyClosure.code, /light-a-marker/);
  assert.doesNotMatch(heavyClosure.code, /light-b-marker/);
}
