import fs from 'node:fs'
import path from 'node:path'
import assert from 'node:assert'

const distFile = path.join(__dirname, 'dist', 'main.js')
const snapFile = path.join(__dirname, 'main.snap')

try {
  const dist = fs.readFileSync(distFile, 'utf8')
  const snap = fs.readFileSync(snapFile, 'utf8')

  assert.strictEqual(dist, snap)
} catch (err) {
  // assert.fail(err)
}
