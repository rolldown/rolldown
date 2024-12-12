import assert from 'node:assert'

assert(import.meta.dirname.includes('nest'))
assert(import.meta.filename.includes('nest'))
assert(import.meta.url.includes('nest'))
