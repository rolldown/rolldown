import assert from 'node:assert';

enum Kind {
  Key = 'composable',
}

const key: string = useKey();
assert.strictEqual(key, Kind.Key);
