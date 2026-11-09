import assert from 'node:assert/strict';
import asset from './file.txt';

const requiredAsset = require('./file.txt');
const url = new URL('./file.txt', import.meta.url);

assert.equal(typeof requiredAsset, 'string');
assert.equal(requiredAsset, asset);
assert.equal(new URL(asset, import.meta.url).href, url.href);
