import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';

// Match static `import`/`export ... from "./x"` and bare `import "./x"`.
// The `\s+` after `import`/`export` excludes dynamic `import("./x")`.
const STATIC_IMPORT_RE = /(?:import|export)\s+(?:[^"']*?\s+from\s+)?["']\.\/([^"']+)["']/g;

function buildStaticImportGraph(distDir) {
  const jsFiles = fs
    .readdirSync(distDir)
    .filter((file) => file.endsWith('.js'))
    .sort();

  const graph = Object.fromEntries(
    jsFiles.map((file) => {
      const code = fs.readFileSync(path.join(distDir, file), 'utf8');
      const imports = [...code.matchAll(STATIC_IMPORT_RE)].map((match) => match[1]);
      return [file, imports];
    }),
  );

  return { jsFiles, graph };
}

// Returns the first cycle as an array of file names (e.g. ['a.js', 'b.js', 'a.js'])
// or `null` if the static import graph is acyclic. Dynamic `import("./x")` is
// intentionally excluded because async boundaries break TDZ hazards.
export function findStaticImportCycle(distDir) {
  const { jsFiles, graph } = buildStaticImportGraph(distDir);

  const seen = new Set();
  function dfs(file, stack) {
    if (seen.has(file)) return null;
    const stackIndex = stack.indexOf(file);
    if (stackIndex !== -1) return stack.slice(stackIndex).concat(file);
    for (const dep of graph[file] ?? []) {
      const cycle = dfs(dep, stack.concat(file));
      if (cycle) return cycle;
    }
    seen.add(file);
    return null;
  }

  for (const file of jsFiles) {
    const cycle = dfs(file, []);
    if (cycle) return { cycle, graph };
  }
  return { cycle: null, graph };
}

// Asserts the static import graph in `distDir` has no cycles. Throws via
// `node:assert` with the offending cycle and the full graph on failure.
export function assertNoStaticImportCycle(distDir) {
  const { cycle, graph } = findStaticImportCycle(distDir);
  assert.strictEqual(
    cycle,
    null,
    cycle
      ? `Static import graph must be acyclic, found: ${cycle.join(' -> ')}\nGraph: ${JSON.stringify(graph)}`
      : '',
  );
}
