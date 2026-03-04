import assert from 'node:assert';
import { BindingMagicString as MagicString } from 'rolldown';
import { describe, it } from 'vitest';

/**
 * Hand-written tests for rolldown-specific BindingMagicString offset behaviour.
 * These are NOT auto-generated; do not delete or regenerate this file.
 */
describe('BindingMagicString offset', () => {
  describe('P1: underflow guard — negative (index + offset) must throw, not panic', () => {
    it('remove() throws when offset causes index underflow', () => {
      const s = new MagicString('hello world', { offset: -1 });
      assert.throws(() => s.remove(0, 1), /out of bounds/);
    });

    it('prependLeft() throws when offset causes index underflow', () => {
      const s = new MagicString('hello world', { offset: -1 });
      assert.throws(() => s.prependLeft(0, 'x'), /out of bounds/);
    });

    it('prependRight() throws when offset causes index underflow', () => {
      const s = new MagicString('hello world', { offset: -1 });
      assert.throws(() => s.prependRight(0, 'x'), /out of bounds/);
    });

    it('appendLeft() throws when offset causes index underflow', () => {
      const s = new MagicString('hello world', { offset: -1 });
      assert.throws(() => s.appendLeft(0, 'x'), /out of bounds/);
    });

    it('appendRight() throws when offset causes index underflow', () => {
      const s = new MagicString('hello world', { offset: -1 });
      assert.throws(() => s.appendRight(0, 'x'), /out of bounds/);
    });

    it('update() throws when offset causes index underflow', () => {
      const s = new MagicString('hello world', { offset: -1 });
      assert.throws(() => s.update(0, 1, 'x'), /out of bounds/);
    });

    it('overwrite() throws when offset causes index underflow', () => {
      const s = new MagicString('hello world', { offset: -1 });
      assert.throws(() => s.overwrite(0, 1, 'x'), /out of bounds/);
    });
  });

  describe('P2: slice() default end resolves to original string end regardless of offset', () => {
    it('slice(explicitStart) with negative offset ends at original string end', () => {
      // offset -1: user index 1 maps to internal 0 ('h')
      const s = new MagicString('hello world', { offset: -1 });
      assert.equal(s.slice(1), 'hello world');
    });

    it('slice(explicitStart, explicitEnd) with negative offset works correctly', () => {
      const s = new MagicString('hello world', { offset: -1 });
      assert.equal(s.slice(1, 6), 'hello');
    });

    it('slice() with positive offset still returns from shifted start to string end', () => {
      // Regression: existing behaviour must be preserved
      const s = new MagicString('hello world', { offset: 1 });
      assert.equal(s.slice(), 'ello world');
    });
  });
});
