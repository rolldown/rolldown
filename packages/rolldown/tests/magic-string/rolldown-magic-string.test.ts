import assert from 'node:assert';
import { RolldownMagicString as MagicString } from 'rolldown';
import { describe, it } from 'vitest';

/**
 * Hand-written tests for rolldown-specific BindingMagicString behaviour.
 * These are NOT auto-generated; do not delete or regenerate this file.
 */

describe('offset', () => {
  describe('underflow guard — negative (index + offset) must throw, not panic', () => {
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

  describe('slice() default end resolves to original string end regardless of offset', () => {
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
      const s = new MagicString('hello world', { offset: 1 });
      assert.equal(s.slice(), 'ello world');
    });
  });
});

describe('original', () => {
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

describe('isRolldownMagicString', () => {
  it('should have isRolldownMagicString on prototype', () => {
    assert.strictEqual(MagicString.prototype.isRolldownMagicString, true);
  });

  it('should be accessible on instances', () => {
    const s = new MagicString('hello');
    assert.strictEqual(s.isRolldownMagicString, true);
  });

  it('should allow detection without importing rolldown', () => {
    const s = new MagicString('hello');
    const isNative = (obj: unknown): boolean =>
      typeof obj === 'object' &&
      obj !== null &&
      (obj as Record<string, unknown>).isRolldownMagicString === true;
    assert.ok(isNative(s));
    assert.ok(!isNative({}));
    assert.ok(!isNative('string'));
    assert.ok(!isNative(null));
  });
});

describe('unicode handling', () => {
  // Exact repro from issue #8685
  it('should slice strings with emoji (surrogate pairs)', () => {
    const s = new MagicString('some 🤷‍♂️ string');
    // '🤷‍♂️' is composed of: 🤷 (U+1F937, 2 UTF-16 units) + ZWJ (U+200D, 1) + ♂ (U+2642, 1) + VS16 (U+FE0F, 1) = 5 UTF-16 units
    // 'some ' = 5 UTF-16 units, so emoji sequence ends at index 10
    assert.strictEqual(s.slice(0, 5), 'some ');
    assert.strictEqual(s.slice(10), ' string');
  });

  it('should overwrite across emoji boundaries', () => {
    const s = new MagicString('a🤷b');
    // 'a' = index 0-1, '🤷' = index 1-3 (2 UTF-16 units), 'b' = index 3-4
    s.overwrite(0, 3, 'replaced');
    assert.strictEqual(s.toString(), 'replacedb');
  });

  it('should remove emoji characters', () => {
    const s = new MagicString('hello🌍world');
    // 'hello' = 0-5, '🌍' = 5-7 (2 UTF-16 units), 'world' = 7-12
    s.remove(5, 7);
    assert.strictEqual(s.toString(), 'helloworld');
  });

  it('should handle CJK characters (3-byte UTF-8, 1 UTF-16 unit)', () => {
    const s = new MagicString('你好世界');
    // Each CJK character is 1 UTF-16 unit
    assert.strictEqual(s.slice(0, 2), '你好');
    assert.strictEqual(s.slice(2, 4), '世界');
  });

  it('should handle mixed ASCII, CJK, and emoji', () => {
    const s = new MagicString('hi你好🌍ok');
    // 'h'=0, 'i'=1, '你'=2, '好'=3, '🌍'=4-5 (surrogate pair), 'o'=6, 'k'=7
    assert.strictEqual(s.slice(0, 2), 'hi');
    assert.strictEqual(s.slice(2, 4), '你好');
    assert.strictEqual(s.slice(6, 8), 'ok');
  });

  it('should handle negative indices with multi-byte characters', () => {
    const s = new MagicString('abc🤷def');
    // Total length: 3 + 2 + 3 = 8 UTF-16 units
    // -3 should map to index 5 => 'def'
    assert.strictEqual(s.slice(-3), 'def');
  });

  it('should handle update with emoji', () => {
    const s = new MagicString('hello🌍world');
    // Replace '🌍' (indices 5-7) with ' '
    s.update(5, 7, ' ');
    assert.strictEqual(s.toString(), 'hello world');
  });

  it('should handle prepend/append left/right with emoji', () => {
    const s = new MagicString('a🤷b');
    // '🤷' starts at index 1, ends at index 3
    s.appendLeft(3, '!');
    assert.strictEqual(s.toString(), 'a🤷!b');
  });

  it('should return lone surrogates when indexing middle of surrogate pair', () => {
    const s = new MagicString('a🤷b');
    // In JS: 'a'=0, high surrogate (0xD83E)=1, low surrogate (0xDD37)=2, 'b'=3
    // slice(1) starts at high surrogate — includes the full emoji
    assert.strictEqual(s.slice(1), '🤷b');
    // slice(2) starts at low surrogate — returns lone low surrogate + 'b'
    // matching JS behavior: 'a🤷b'.slice(2) === '\uDD37b'
    assert.strictEqual(s.slice(2), '\uDD37b');
    // slice(0, 2) ends at low surrogate — returns 'a' + lone high surrogate
    // matching JS behavior: 'a🤷b'.slice(0, 2) === 'a\uD83E'
    assert.strictEqual(s.slice(0, 2), 'a\uD83E');
    // slice(3) is 'b'
    assert.strictEqual(s.slice(3), 'b');
  });

  it('should return an empty string for slice(i, i) at a low-surrogate index', () => {
    const s = new MagicString('a🤷b');
    assert.strictEqual(s.slice(2, 2), '');
  });

  it('should return an empty string for reversed ranges across a surrogate boundary', () => {
    const s = new MagicString('a🤷b');
    assert.strictEqual(s.slice(2, 1), '');
  });
});
