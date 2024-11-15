
import assert from 'node:assert'

export const url = new URL('./node_modules/foo/index.txt', import.meta.url);

export const url2 = new URL('node_modules/foo/index.txt', import.meta.url);

export const url3 = new URL('foo', import.meta.url);

assert.strictEqual(url.href, url3.href);
assert.strictEqual(url2.href, url3.href);