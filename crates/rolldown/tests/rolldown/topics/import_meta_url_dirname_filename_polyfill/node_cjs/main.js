import assert from 'node:assert';

assert.equal(require("url").pathToFileURL(__filename), import.meta.url)
assert.equal(__dirname, import.meta.dirname)
assert.equal(__filename, import.meta.filename)
