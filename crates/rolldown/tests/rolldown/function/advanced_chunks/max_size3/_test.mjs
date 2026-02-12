import fs from 'node:fs'
import path from 'node:path'
import assert from 'node:assert'

const dist = path.join(import.meta.dirname, 'dist')
const files = fs.readdirSync(dist).filter(f => f !== 'package.json').sort()

// maxSize=42 should split into two chunks: [size-15 + size-20] and [size-41]
assert.deepStrictEqual(files, ['42max-size.js', '42max-size2.js', 'main.js'])

const chunk1 = fs.readFileSync(path.join(dist, '42max-size.js'), 'utf-8')
assert.ok(chunk1.includes('size-15.js'), 'chunk1 should contain size-15')
assert.ok(chunk1.includes('size-20.js'), 'chunk1 should contain size-20')
assert.ok(!chunk1.includes('size-41.js'), 'chunk1 should not contain size-41')

const chunk2 = fs.readFileSync(path.join(dist, '42max-size2.js'), 'utf-8')
assert.ok(chunk2.includes('size-41.js'), 'chunk2 should contain size-41')
assert.ok(!chunk2.includes('size-15.js'), 'chunk2 should not contain size-15')
assert.ok(!chunk2.includes('size-20.js'), 'chunk2 should not contain size-20')
