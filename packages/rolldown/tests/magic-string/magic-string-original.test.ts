import assert from 'node:assert';
import { RolldownMagicString as MagicString } from 'rolldown';
import { describe, it } from 'vitest';

describe('MagicString#original', () => {
  it('should return the original source string', () => {
    const s = new MagicString('hello world');
    assert.strictEqual(s.original, 'hello world');
  });

  it('should not change after modifications', () => {
    const s = new MagicString('hello world');
    s.overwrite(0, 5, 'goodbye');
    assert.strictEqual(s.original, 'hello world');
    assert.strictEqual(s.toString(), 'goodbye world');
  });

  it('should return full source string regardless of offset', () => {
    const s = new MagicString('hello world', { offset: 6 });
    assert.strictEqual(s.original, 'hello world');
  });

  it('should be preserved after clone', () => {
    const s = new MagicString('hello world');
    s.overwrite(0, 5, 'goodbye');
    const clone = s.clone();
    assert.strictEqual(clone.original, 'hello world');
  });
});
