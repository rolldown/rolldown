import default_default from './dist/default.js'
import assert from 'node:assert'


assert.strictEqual(default_default, 42, 'default export should be preserved as default export');


