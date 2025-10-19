import nodePath from 'node:path';
import nodeFs from 'node:fs';
import nodeAssert from 'node:assert';

const distDir = nodePath.join(import.meta.dirname, 'dist');
const entryFile = nodePath.join(distDir, 'entries', 'main-esm.js');
nodeAssert(
  nodeFs.existsSync(entryFile),
  `expected entry chunk ${entryFile} to exist`,
);

const chunkDir = nodePath.join(distDir, 'chunks');
const chunkFiles = nodeFs.readdirSync(chunkDir);
nodeAssert(
  chunkFiles.some((file) => file.endsWith('-esm.js')),
  'expected a chunk filename with "-esm.js"',
);

nodeAssert(
  !chunkFiles.some((file) => file.includes('[format]')),
  'chunk filenames should not keep the literal "[format]" placeholder',
);
