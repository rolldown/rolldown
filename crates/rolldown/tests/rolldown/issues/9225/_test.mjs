import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';

const distDir = path.join(import.meta.dirname, 'dist');
const jsFiles = fs
  .readdirSync(distDir)
  .filter((file) => file.endsWith('.js'))
  .sort();

const graph = Object.fromEntries(
  jsFiles.map((file) => {
    const code = fs.readFileSync(path.join(distDir, file), 'utf8');
    const imports = [
      ...code.matchAll(/(?:import|export)\s+(?:[^"']*?\s+from\s+)?["']\.\/([^"']+)["']/g),
    ].map((match) => match[1]);
    return [file, imports];
  }),
);

function findCycle() {
  const visited = new Set();
  const inStack = new Set();

  function dfs(file, pathSoFar) {
    if (inStack.has(file)) {
      return pathSoFar.slice(pathSoFar.indexOf(file)).concat(file);
    }
    if (visited.has(file)) {
      return null;
    }
    visited.add(file);
    inStack.add(file);
    for (const dep of graph[file] ?? []) {
      const cycle = dfs(dep, pathSoFar.concat(file));
      if (cycle) {
        return cycle;
      }
    }
    inStack.delete(file);
    return null;
  }

  for (const file of Object.keys(graph)) {
    const cycle = dfs(file, []);
    if (cycle) {
      return cycle;
    }
  }
  return null;
}

assert.strictEqual(
  findCycle(),
  null,
  `Output chunks must not have circular static imports: ${JSON.stringify(graph)}`,
);

await import('./dist/main.js');
assert.strictEqual(globalThis.__rolldown_issue_7449_value, 300000);
assert.strictEqual(globalThis.__rolldown_issue_7449_side, 1);

await Promise.all(globalThis.__rolldown_issue_7449_imports);
assert.strictEqual(globalThis.__rolldown_issue_7449_side, 1);
