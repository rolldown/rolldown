import nodePath from 'node:path';
import { prepareGraphCorpus } from './graph-corpus.mjs';

const options = new Map();
for (let index = 2; index < process.argv.length; index++) {
  const argument = process.argv[index];
  if (argument === '--update-manifest') {
    options.set(argument, true);
    continue;
  }
  const value = process.argv[++index];
  if (!value) throw new Error(`expected a value after ${argument}`);
  options.set(argument, value);
}
const sourceRoot = options.get('--source');
if (!sourceRoot) throw new Error('expected --source <pinned shadcn-svelte checkout>');
const destination = nodePath.resolve(
  options.get('--destination') ?? nodePath.join(import.meta.dirname, '.graph-corpus'),
);
const manifestPath = nodePath.join(import.meta.dirname, 'source-manifest.json');
const manifest = await prepareGraphCorpus({
  sourceRoot: nodePath.resolve(sourceRoot),
  destination,
  manifestPath,
  updateManifest: options.has('--update-manifest'),
});
console.log(
  JSON.stringify({
    destination,
    files: manifest.summary.files,
    bytes: manifest.summary.bytes,
    entries: manifest.entryPaths.length,
    aggregateSha256: manifest.summary.aggregateSha256,
  }),
);
