// @ts-nocheck
import assert from 'node:assert';
import { json, name } from './dist/main';

assert(name === '@test-fixture/named-exports');
assert(name === json.name);
assert(json.const === true);
