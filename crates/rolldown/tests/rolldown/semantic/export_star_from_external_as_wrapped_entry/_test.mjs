import { readFile as readFile2, main } from './dist/entry.js'
import { readFile } from 'node:fs'
import assert from 'assert'
assert.strictEqual(main, 'main')
assert.strictEqual(readFile, readFile2)
