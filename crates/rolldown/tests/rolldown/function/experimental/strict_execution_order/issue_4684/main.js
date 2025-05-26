import nodeAssert from 'node:assert'

import { setup } from './setup.js';
setup();

import { read } from './read.js';
read();

import('./dynamic.js');


nodeAssert.strictEqual(globalThis.foo, 'foo')