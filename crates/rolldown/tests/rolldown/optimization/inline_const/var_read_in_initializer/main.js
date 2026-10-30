import assert from 'node:assert';

export var flag = ((flag || (() => {}))(), true);

const reads = [];
export var value = (reads.push(value), true);

assert.deepStrictEqual(reads, [undefined]);
