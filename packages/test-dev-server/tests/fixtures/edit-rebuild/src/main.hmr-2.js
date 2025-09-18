// @restart
import assert from 'node:assert';
import nodeFs from 'node:fs';
import { value as fooValue } from './foo';

assert.strictEqual(fooValue, 'edited-foo');
nodeFs.writeFileSync('./ok-1', '');
