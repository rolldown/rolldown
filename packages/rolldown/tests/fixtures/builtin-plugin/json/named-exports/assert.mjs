// @ts-nocheck
import assert from 'node:assert'
import { name, json } from './dist/main'

assert(name === 'stringify')
assert(name === json.name)
