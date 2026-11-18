// @ts-nocheck
import assert from 'node:assert';
import { modules } from './dist/main';

assert.deepStrictEqual(modules, { '../docs/a.js': 'a' });
