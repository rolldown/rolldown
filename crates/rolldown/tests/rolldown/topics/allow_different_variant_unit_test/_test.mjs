import * as fs from 'node:fs'
import path from 'node:path'
import assert from 'node:assert'


const file = fs.readFileSync(path.resolve(import.meta.dirname, "./dist/main.js"), "utf-8")

if (globalThis.__testName === 'enable-treeshake') {
  assert.ok(!file.includes('test'))
} else {
  assert.ok(file.includes('test'))
}
