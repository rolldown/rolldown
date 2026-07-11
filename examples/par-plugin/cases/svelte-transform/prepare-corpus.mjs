import nodePath from 'node:path';
import { prepareCorpus } from './corpus.mjs';

const argumentsByName = new Map();
for (let index = 2; index < process.argv.length; index++) {
  const argument = process.argv[index];
  if (argument === '--update-manifest') {
    argumentsByName.set(argument, true);
    continue;
  }
  const value = process.argv[++index];
  if (!value) throw new Error(`expected a value after ${argument}`);
  argumentsByName.set(argument, value);
}

const sourceRoot = argumentsByName.get('--source');
if (!sourceRoot) throw new Error('expected --source <pinned shadcn-svelte checkout>');
const destination = nodePath.resolve(
  argumentsByName.get('--destination') ?? nodePath.join(import.meta.dirname, '.corpus'),
);
const manifestPath = nodePath.join(import.meta.dirname, 'corpus-manifest.json');
const manifest = await prepareCorpus({
  sourceRoot: nodePath.resolve(sourceRoot),
  destination,
  manifestPath,
  updateManifest: argumentsByName.has('--update-manifest'),
});
console.log(
  JSON.stringify({
    destination,
    files: manifest.summary.files,
    bytes: manifest.summary.bytes,
    aggregateSha256: manifest.summary.aggregateSha256,
  }),
);
