import { readFile as readFile2 } from './dist/entry.mjs'
import { readFile } from 'node:fs'
import assert from 'assert'
assert.strictEqual(readFile, readFile2)
