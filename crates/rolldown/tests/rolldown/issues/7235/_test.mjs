import nodeFs from 'node:fs';
import nodePath from 'node:path';
import assert from 'node:assert';

const distDir = nodePath.join(import.meta.dirname, 'dist');
const mainFile = nodePath.join(distDir, 'main.js');
const content = nodeFs.readFileSync(mainFile, 'utf8');

// The dead branch should be eliminated by DCE
assert(!content.includes('should not see me in bundle'), 'dead branch should be tree-shaken');
// The live branch should remain
assert(content.includes('flag is false correctly'), 'live branch should be kept');
