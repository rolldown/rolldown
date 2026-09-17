import assert from 'node:assert';
import nativePaths from './native-paths.cjs';

assert.equal(require('url').pathToFileURL(__filename), import.meta.url);
assert.equal(__dirname, import.meta.dirname);
assert.equal(__filename, import.meta.filename);

// computed, optional and optional-computed forms should be polyfilled the same way
assert.equal(require('url').pathToFileURL(__filename).href, import.meta['url']);
assert.equal(require('url').pathToFileURL(__filename).href, import.meta?.url);
assert.equal(require('url').pathToFileURL(__filename).href, import.meta?.['url']);
assert.equal(__dirname, import.meta['dirname']);
assert.equal(__filename, import.meta?.filename);
assert.deepEqual(nativePaths, [__dirname, __filename]);
