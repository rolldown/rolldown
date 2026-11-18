import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import {
  direct,
  separate,
  separateValue,
  second,
  third,
  excluded,
  mixed,
  localValue,
} from './dist/main.js';

assert.deepEqual(
  { direct, separate, separateValue, second, third, excluded, mixed, localValue },
  {
    direct: { value: 42, default: 'default' },
    separate: { value: 42, second: 2, third: 3, restOnly: 'rest-only', default: 'default' },
    separateValue: 42,
    second: 2,
    third: 3,
    excluded: 'excluded',
    mixed: { retained: 'retained', default: 'default' },
    localValue: 'kept',
  },
);

for (const file of readdirSync(new URL('./dist/', import.meta.url))) {
  if (file.endsWith('.js')) {
    assert.doesNotMatch(
      readFileSync(new URL(`./dist/${file}`, import.meta.url), 'utf8'),
      /unused-rest-export/,
    );
  }
}
