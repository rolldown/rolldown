import assert from 'node:assert'
import fs from 'node:fs/promises'
const content = await fs.readFile(new URL('dist/main.js', import.meta.url), 'utf8')
assert(!content.includes('"use strict"'))