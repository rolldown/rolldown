import nodePath from 'node:path';
import { REPOSITORIES, prepareCorpus } from './corpus.mjs';

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

const sourcesDirectory = argumentsByName.get('--sources');
const sourceRoots = Object.fromEntries(
  REPOSITORIES.map(({ id }) => {
    const explicit = argumentsByName.get(`--${id}`);
    const sourceRoot =
      explicit ?? (sourcesDirectory ? nodePath.join(sourcesDirectory, id) : undefined);
    if (!sourceRoot) {
      throw new Error(`expected --${id} <checkout> or --sources <directory>`);
    }
    return [id, nodePath.resolve(sourceRoot)];
  }),
);
const destination = nodePath.resolve(
  argumentsByName.get('--destination') ?? nodePath.join(import.meta.dirname, '.corpus'),
);
const manifestPath = nodePath.join(import.meta.dirname, 'corpus-manifest.json');
const manifest = await prepareCorpus({
  sourceRoots,
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
    selections: Object.fromEntries(
      Object.entries(manifest.selections).map(([count, selection]) => [
        count,
        selection.selectionSha256,
      ]),
    ),
  }),
);
