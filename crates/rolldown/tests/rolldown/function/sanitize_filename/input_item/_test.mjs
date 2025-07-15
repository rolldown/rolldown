import fs from 'node:fs'
import path from 'node:path'
import assert from 'node:assert'

assert.ok(fs.existsSync(path.join(import.meta.dirname, 'dist/_main.js')))