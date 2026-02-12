import fs from 'node:fs'
import path from 'node:path'
import assert from 'node:assert'

const dist = path.join(import.meta.dirname, 'dist')
const files = fs.readdirSync(dist).filter(f => f !== 'package.json').sort()

// maxSize=14 is smaller than every module, so each file becomes its own chunk
assert.deepStrictEqual(files, ['14max-size.js', '14max-size2.js', '14max-size3.js', 'main.js'])

const chunk1 = fs.readFileSync(path.join(dist, '14max-size.js'), 'utf-8')
assert.ok(chunk1.includes('size-15.js'), 'chunk1 should contain size-15')

const chunk2 = fs.readFileSync(path.join(dist, '14max-size2.js'), 'utf-8')
assert.ok(chunk2.includes('size-20.js'), 'chunk2 should contain size-20')

const chunk3 = fs.readFileSync(path.join(dist, '14max-size3.js'), 'utf-8')
assert.ok(chunk3.includes('size-41.js'), 'chunk3 should contain size-41')
