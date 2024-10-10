import nodeAssert from 'node:assert'
import { importOutput, requireOutput } from './dist/main.js'
nodeAssert.strictEqual(importOutput, 'esm')
nodeAssert.strictEqual(requireOutput, 'cjs')
