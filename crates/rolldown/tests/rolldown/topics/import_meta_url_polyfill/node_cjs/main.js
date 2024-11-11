import assert from 'node:assert';

assert.equal(require("url").pathToFileURL(__filename), import.meta.url)