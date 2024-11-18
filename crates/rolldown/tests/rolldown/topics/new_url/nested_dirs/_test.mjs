import assert from 'node:assert'
import { url, dep } from './dist/main.js'

const depExports = await dep

assert.equal(url.href, depExports.url.href)

