import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert';

/**
 * Parse static import/export specifiers from a chunk's source code.
 * Only captures relative imports (starting with `./`).
 */
function getStaticImports(source) {
  const imports = [];
  const re = /(?:import|export)\s+.*?from\s+["']\.\/([^"']+)["']/g;
  let m;
  while ((m = re.exec(source)) !== null) {
    imports.push(m[1]);
  }
  return imports;
}

/**
 * Build a static-import dependency graph from all `.js` files in `distDir`,
 * then assert that no circular imports exist.
 *
 * @param {string} distDir - Absolute path to the dist directory.
 */
export function assertNoCircularImports(distDir) {
  const files = fs.readdirSync(distDir).filter((f) => f.endsWith('.js'));
  const graph = {};
  for (const file of files) {
    const source = fs.readFileSync(path.join(distDir, file), 'utf-8');
    graph[file] = getStaticImports(source);
  }

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

  let cycle = null;
  for (const node of Object.keys(graph)) {
    cycle = dfs(node, []);
    if (cycle) break;
  }

  assert.strictEqual(
    cycle,
    null,
    `Output chunks must not have circular static imports, but found: ${cycle ? cycle.join(' -> ') : 'none'}`,
  );
}
