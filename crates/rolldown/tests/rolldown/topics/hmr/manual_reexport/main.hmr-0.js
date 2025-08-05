import assert from "node:assert"
import { Globals } from './foo.js'

assert.strictEqual(Globals, Object);

import.meta.hot.accept()
