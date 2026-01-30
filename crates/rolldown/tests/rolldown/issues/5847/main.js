import assert from 'node:assert'

const nlp = (await import('./compromise')).default
assert.strictEqual(nlp.extend(), 666)