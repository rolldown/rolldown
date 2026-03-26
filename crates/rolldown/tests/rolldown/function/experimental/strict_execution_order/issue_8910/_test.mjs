import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert';

const dist = path.join(import.meta.dirname, 'dist');

// Parse static imports from a chunk's source code
function getStaticImports(source) {
  const imports = [];
  const re = /(?:import|export)\s+.*?from\s+["']\.\/([^"']+)["']/g;
  let m;
  while ((m = re.exec(source)) !== null) {
    imports.push(m[1]);
  }
  return imports;
}

// Build a dependency graph of static imports between output chunks
const files = fs.readdirSync(dist).filter((f) => f.endsWith('.js'));
const graph = {};
for (const file of files) {
  const source = fs.readFileSync(path.join(dist, file), 'utf-8');
  graph[file] = getStaticImports(source);
}

// Detect cycles in the static import graph using DFS
function findCycle(graph) {
  const visited = new Set();
  const inStack = new Set();

  function dfs(node, pathSoFar) {
    if (inStack.has(node)) {
      const cycleStart = pathSoFar.indexOf(node);
      return pathSoFar.slice(cycleStart).concat(node);
    }
    if (visited.has(node)) return null;

    visited.add(node);
    inStack.add(node);
    for (const dep of graph[node] || []) {
      const cycle = dfs(dep, [...pathSoFar, node]);
      if (cycle) return cycle;
    }
    inStack.delete(node);
    return null;
  }

  for (const node of Object.keys(graph)) {
    const cycle = dfs(node, []);
    if (cycle) return cycle;
  }
  return null;
}

const cycle = findCycle(graph);
assert.strictEqual(
  cycle,
  null,
  `Output chunks must not have circular static imports, but found: ${cycle ? cycle.join(' -> ') : 'none'}`,
);
