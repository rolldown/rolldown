import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert';

// Regression test for runtime helper circular dependency.
//
// When rolldown places the runtime module (containing __exportAll, __commonJSMin, etc.)
// inside the entry chunk, and facade elimination adds __exportAll to a common chunk,
// that common chunk may import __exportAll from the entry chunk. If the entry chunk also
// imports from the common chunk, a circular dependency is created, causing the helper
// to be `undefined` at ESM evaluation time.
//
// The fix extracts the runtime module into a standalone chunk when the runtime is in
// an entry chunk and other chunks need runtime helpers, breaking the potential cycle.

const distDir = path.join(import.meta.dirname, 'dist');
const files = fs.readdirSync(distDir).filter((f) => f.endsWith('.js'));

for (const file of files) {
  const content = fs.readFileSync(path.join(distDir, file), 'utf8');
  // No chunk should import runtime helpers (like __exportAll) from the entry chunk
  // when the entry chunk also imports from that chunk — that would be a circular dependency.
  if (file !== 'entry.js' && content.includes('from "./entry.js"')) {
    const entryContent = fs.readFileSync(path.join(distDir, 'entry.js'), 'utf8');
    const entryImportsFromThis = entryContent.includes(`from "./${file}"`);
    assert(
      !entryImportsFromThis,
      `Circular dependency: ${file} imports from entry.js AND entry.js imports from ${file}`,
    );
  }
}
