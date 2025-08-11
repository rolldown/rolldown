import fs from 'node:fs'
import path from 'node:path'
import assert from 'node:assert'

const bundledFile = fs.readFileSync(path.resolve(import.meta.dirname, 'dist/main.js'))
assert(bundledFile.includes('./.x.js'))
assert(bundledFile.includes('./..y.js'))
