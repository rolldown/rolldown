import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert';

const distDir = path.join(import.meta.dirname, 'dist');
const isStrictExecutionOrder = globalThis.__configName === 'strict_execution_order';

if (isStrictExecutionOrder) {
  // With strictExecutionOrder, the circular dependency check is skipped,
  // so the facade chunk is eliminated and a.js contains the actual content.
  assert(
    !fs.existsSync(path.join(distDir, 'a2.js')),
    'a2.js should not exist (facade was eliminated)',
  );
  const aContent = fs.readFileSync(path.join(distDir, 'a.js'), 'utf8');
  assert(!aContent.includes('import "./a2.js"'), 'a.js should not be a facade');
} else {
  // Without strictExecutionOrder, the circular dependency check prevents
  // the facade from being eliminated. a.js is a facade that only imports a2.js for side effects.
  assert(
    fs.existsSync(path.join(distDir, 'a2.js')),
    'a2.js common chunk should exist (facade was not eliminated)',
  );
  const aContent = fs.readFileSync(path.join(distDir, 'a.js'), 'utf8');
  assert(aContent.includes('import "./a2.js"'), 'a.js should be a facade importing a2.js');
}
