import { readFileSync } from 'node:fs';

const staticChunk = readFileSync(new URL('./dist/static.js', import.meta.url), 'utf8');

if (staticChunk.includes('"./supported.js"')) {
  throw new Error(
    'static chunks must not import CommonJS wrappers back from their importing entry',
  );
}
