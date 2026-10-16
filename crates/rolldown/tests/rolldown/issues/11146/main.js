import assert from 'node:assert';
import { env } from './dynamic.js';
import * as ns from './nested.js';
import * as server from './server.js';

// Native ESM throws `TypeError` for each write, so the output must keep the write on the namespace object.
assert.throws(() => {
  env.FOO = 'changed';
}, TypeError);
assert.throws(() => {
  env.FOO++;
}, TypeError);
assert.throws(() => {
  [env.FOO] = ['changed'];
}, TypeError);
assert.throws(() => {
  for (env.FOO of ['changed']);
}, TypeError);
assert.throws(() => {
  delete env.FOO;
}, TypeError);
assert.throws(() => {
  delete env?.FOO;
}, TypeError);
assert.throws(() => {
  ns.sub.FOO = 'changed';
}, TypeError);
assert.throws(() => {
  server.FOO = 'changed';
}, TypeError);
assert.throws(() => {
  delete server.FOO;
}, TypeError);
assert.strictEqual(env.FOO, 'foo');
