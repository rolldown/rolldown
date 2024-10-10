import { readFile as readFile2 } from './dist/entry.js'
import { readFile as readFile3 } from './dist/entry2.js'
import { readFile } from 'node:fs'
import assert from 'assert'
assert.strictEqual(readFile, readFile2)
assert.strictEqual(readFile, readFile3)
