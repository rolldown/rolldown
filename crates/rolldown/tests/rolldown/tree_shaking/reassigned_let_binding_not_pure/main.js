import assert from 'node:assert';
import { setup } from './logger.js';
import { doWork } from './consumer.js';

setup();
doWork();

assert.strictEqual(globalThis.result, 'hello from doWork');
