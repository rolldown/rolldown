import assert from 'node:assert';
import { RolldownMagicString as MagicString } from 'rolldown';
import { describe, it } from 'vitest';

/**
 * Hand-written tests for rolldown-specific BindingMagicString behaviour.
 * These are NOT auto-generated; do not delete or regenerate this file.
 *
 * The `string_wizard: replace` block near the bottom was ported from the plain
 * `replace` / `replaceAll` unit tests in
 * `crates/string_wizard/tests/magic_string_replace.rs`. Running them through the
 * JS `RolldownMagicString` binding exercises the full pipeline — in particular
 * the UTF-16 (JS string index) -> UTF-8 (byte offset) location conversion that
 * only exists on the binding boundary. The rest of the string_wizard crate
 * tests stay in Rust.
 */

describe('length', () => {
  // `MagicString::len()` sums UTF-8 byte lengths, but JS measures string length in UTF-16
  // code units. The binding used to return the byte count, so every non-ASCII source
  // over-reported (`'é'` -> 2, `'🤷'` -> 4). The upstream suite only covers ASCII, where
  // the two happen to agree, so nothing caught it.
  it('counts UTF-16 code units, not UTF-8 bytes', () => {
    assert.strictEqual(new MagicString('abc').length(), 3);
    assert.strictEqual(new MagicString('é').length(), 1);
    assert.strictEqual(new MagicString('🤷').length(), 2);
    assert.strictEqual(new MagicString('中文').length(), 2);
    assert.strictEqual(new MagicString('abc™def').length(), 7);
  });

  it('counts UTF-16 code units after edits', () => {
    const s = new MagicString('abc');
    s.overwrite(1, 2, '🤷');
    // 'a' + '🤷' (2 units) + 'c'
    assert.strictEqual(s.length(), 4);
    assert.strictEqual(s.length(), s.toString().length);
  });

  it('excludes global intro/outro, matching magic-string', () => {
    const s = new MagicString('abc');
    s.prepend('🤷');
    s.append('🤷');
    assert.strictEqual(s.length(), 3);
  });

  // An empty source still has one `[0, 0)` chunk, and index 0 is both its start and its end —
  // so a positional insert there belongs *on the chunk* and counts, exactly as it does in
  // magic-string. `by_start_mut`/`by_end_mut` used to short-circuit on `index == source.len()`
  // / `index == 0` and push the content to the global intro/outro instead, where `length()`
  // cannot see it.
  describe('positional inserts at index 0 on an empty source', () => {
    for (const method of ['appendLeft', 'appendRight', 'prependLeft', 'prependRight'] as const) {
      it(`${method}(0, ...) is counted`, () => {
        const s = new MagicString('');
        s[method](0, 'é');
        assert.strictEqual(s.toString(), 'é');
        assert.strictEqual(s.length(), 1);
        assert.strictEqual(s.isEmpty(), false);
      });
    }

    it('counts UTF-16 code units, not bytes, for the inserted content', () => {
      const s = new MagicString('');
      s.appendLeft(0, '🤷');
      assert.strictEqual(s.length(), 2);
    });

    // The contrast: `append`/`prepend` are not positional, so they land in the global
    // intro/outro and stay excluded — on an empty source as anywhere else.
    for (const method of ['append', 'prepend'] as const) {
      it(`${method}(...) stays excluded`, () => {
        const s = new MagicString('');
        s[method]('é');
        assert.strictEqual(s.toString(), 'é');
        assert.strictEqual(s.length(), 0);
        assert.strictEqual(s.isEmpty(), true);
      });
    }
  });

  // The short-circuits were right for a non-empty source: nothing ends at 0 and nothing starts
  // at `source.len()`, so those inserts do belong in the global intro/outro. Removing them must
  // not change that.
  it('keeps out-of-chunk-range inserts on a non-empty source excluded', () => {
    const left = new MagicString('abc');
    left.appendLeft(0, 'X');
    assert.strictEqual(left.toString(), 'Xabc');
    assert.strictEqual(left.length(), 3);

    const right = new MagicString('abc');
    right.appendRight(3, 'X');
    assert.strictEqual(right.toString(), 'abcX');
    assert.strictEqual(right.length(), 3);
  });
});

// One case per branch of `has_changed`'s two-clause condition. The `is true` cases are not
// redundant with the bug case: this change *removes* a fast path, and a suite that only pinned
// the `false` result would accept a `has_changed()` that always returns false.
describe('hasChanged', () => {
  // The fast path compared `source.len()` against `len()`, but `len()` excludes the global
  // intro/outro that `to_string()` includes, so edits that cancel out reported a change.
  // Here the lengths match and the strings match: no change.
  it('is false when edits cancel out to the original string', () => {
    const s = new MagicString('abc');
    s.remove(0, 1);
    s.prepend('a');
    assert.strictEqual(s.toString(), 'abc');
    assert.strictEqual(s.hasChanged(), false);
  });

  // Lengths match, so the fast path defers to the string comparison, which differs.
  it('is true for a real change', () => {
    const s = new MagicString('abc');
    s.overwrite(0, 3, 'XYZ');
    assert.strictEqual(s.hasChanged(), true);
  });

  // The only case where the fast path itself fires: the global intro makes the output longer
  // than the source, which the old `len()` comparison could not see.
  it('is true when only the global intro changed', () => {
    const s = new MagicString('abc');
    s.prepend('x');
    assert.strictEqual(s.hasChanged(), true);
  });
});

describe('splitting an already-edited chunk', () => {
  // These four used to abort the process: `append_left`/`append_right`/`prepend_left`/
  // `prepend_right` called `.expect("unexpected split error")` on the assumption that
  // appends never split an edited chunk. They do, and a Rust panic across the FFI boundary
  // takes the whole process down instead of surfacing as a catchable error.
  // magic-string throws `Cannot split a chunk that has already been edited`.
  for (const method of ['appendLeft', 'appendRight', 'prependLeft', 'prependRight'] as const) {
    it(`${method}() throws instead of panicking`, () => {
      const s = new MagicString('abcdef');
      s.overwrite(0, 6, 'XYZ');
      assert.throws(() => s[method](3, '!'), /already been edited/);
    });
  }

  it('leaves the instance usable after the error', () => {
    const s = new MagicString('abcdef');
    s.overwrite(0, 6, 'XYZ');
    assert.throws(() => s.appendLeft(3, '!'), /already been edited/);
    assert.strictEqual(s.toString(), 'XYZ');
  });

  // A UTF-16 index inside a surrogate pair (index 1 of '🤷') has no byte equivalent, so the
  // index-to-byte mapper rounds it to the character's end — which is the edited chunk's
  // *boundary*, silently bypassing the split error above. magic-string splits mid-pair and
  // throws just the same as at any other interior position.
  for (const method of ['appendLeft', 'appendRight', 'prependLeft', 'prependRight'] as const) {
    it(`${method}() throws for a surrogate-pair position inside an edited chunk`, () => {
      const s = new MagicString('🤷');
      s.overwrite(0, 2, 'X');
      assert.throws(() => s[method](1, '!'), /already been edited/);
    });
  }

  // Contrast, passes either way by design: on *unedited* content magic-string splits the
  // pair into lone surrogates, which UTF-8 cannot represent — rounding to the character
  // boundary is our documented stand-in. Throwing on every surrogate-pair position would
  // regress inserts magic-string accepts.
  it('appendLeft() at a surrogate-pair position on unedited content still succeeds', () => {
    const s = new MagicString('🤷');
    s.appendLeft(1, '!');
    assert.strictEqual(s.toString(), '🤷!');
  });
});

describe('move', () => {
  // A move whose range is no longer contiguous (an earlier move reordered the chunks) has to be
  // rejected before any pointers are rewired. Bailing out mid-rewire left a chunk pointing at
  // itself, so the next toString() spun forever.
  it('rejects a non-contiguous range without corrupting the instance', () => {
    const s = new MagicString('abc');
    s.move(0, 1, 2);
    assert.strictEqual(s.toString(), 'bac');
    assert.throws(() => s.move(1, 3, 0), /spans the entire string/);
    assert.strictEqual(s.toString(), 'bac');
  });
});

describe('storeName', () => {
  // string_wizard supports `keep_original` end-to-end, but the binding hardcoded it to
  // `false`, so `generateMap().names` was always empty regardless of the option.
  // The basic overwrite case lives in the vendored suite ('should recover original names',
  // MagicString.test.ts); these cover only what that test does not reach.

  // `update` has its own napi options struct (`BindingUpdateOptions`) — a missing
  // `store_name` field there is invisible to every overwrite-based test.
  it('update({ storeName: true }) records the original name', () => {
    const s = new MagicString('var foo = 1;');
    s.update(4, 7, 'bar', { storeName: true });
    assert.deepStrictEqual(s.generateMap({}).names, ['foo']);
  });

  // These two pass either way by design: an implementation with no storeName support at all
  // satisfies them trivially. They pin the flag-gating — an implementation that stored every
  // replaced range unconditionally would pass every positive test in this block. Omitted and
  // explicit `false` are distinct deserialization branches (`None` vs `Some(false)`).
  it('names stay empty when storeName is omitted', () => {
    const s = new MagicString('var foo = 1;');
    s.overwrite(4, 7, 'bar');
    assert.deepStrictEqual(s.generateMap({}).names, []);
  });

  it('names stay empty when storeName is false', () => {
    const s = new MagicString('var foo = 1;');
    s.overwrite(4, 7, 'bar', { storeName: false });
    assert.deepStrictEqual(s.generateMap({}).names, []);
  });

  it('contentOnly and storeName are independent', () => {
    const s = new MagicString('var foo = 1;');
    s.appendLeft(4, '/*x*/');
    s.overwrite(4, 7, 'bar', { storeName: true, contentOnly: true });
    assert.deepStrictEqual(s.generateMap({}).names, ['foo']);
    // contentOnly preserves the surrounding intro/outro
    assert.strictEqual(s.toString(), 'var /*x*/bar = 1;');
  });

  // The name recorded is the whole replaced range, independent of chunk boundaries. When an
  // earlier edit has already split that range, the start chunk covers only part of it — using
  // the chunk's own span stored 'f' instead of 'foo', and pointed the mapping at that wrong
  // name. magic-string keys `storedNames` off `original.slice(start, end)`.
  it('records the whole range when an earlier appendLeft split it', () => {
    const s = new MagicString('var foo = 1;');
    s.appendLeft(5, 'X');
    s.overwrite(4, 7, 'bar', { storeName: true });
    assert.deepStrictEqual(s.generateMap({}).names, ['foo']);
  });

  // A range split across chunks leaves the name in `names` with no mapping referencing it —
  // matching magic-string, whose per-mapping lookup is `names.indexOf(chunk.original)` and so
  // misses once the chunk is narrower than the range.
  it('a split range leaves the mapping unnamed, like magic-string', () => {
    const split = new MagicString('var foo = 1;');
    split.appendLeft(5, 'X');
    split.overwrite(4, 7, 'bar', { storeName: true });
    assert.deepStrictEqual(split.generateDecodedMap({}).mappings, [
      [
        [0, 0, 0, 0],
        [4, 0, 0, 4],
        [7, 0, 0, 7],
      ],
    ]);

    // Unsplit, the chunk spans the whole range, so the mapping does carry the name.
    const whole = new MagicString('var foo = 1;');
    whole.overwrite(4, 7, 'bar', { storeName: true });
    assert.deepStrictEqual(whole.generateDecodedMap({}).mappings, [
      [
        [0, 0, 0, 0],
        [4, 0, 0, 4, 0],
        [7, 0, 0, 7],
      ],
    ]);
  });
});

describe('offset', () => {
  describe('underflow guard — negative (index + offset) must throw, not panic', () => {
    it('remove() throws when offset causes index underflow', () => {
      const s = new MagicString('hello world', { offset: -1 });
      assert.throws(() => s.remove(0, 1), /end must be greater than start/);
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

describe('overwrite test extension', () => {
  it('clears interior intro/outro even with `contentOnly: true` (matches JS magic-string)', () => {
    const s = new MagicString('abcdefg');

    s.appendLeft(5, 'X');
    s.prependRight(5, 'Y');
    s.overwrite(1, 6, '...', { contentOnly: true });

    // JS magic-string always clears interior chunks' intro/outro.
    // Only the first chunk's intro/outro is preserved by contentOnly.
    assert.strictEqual(s.toString(), 'a...Xg');
  });

  it('overwrite across a split point throws', () => {
    const s = new MagicString('abcdefghijkl');

    s.move(6, 9, 3);
    s.appendLeft(5, 'foo');

    assert.strictEqual(s.toString(), 'abcghidefoofjkl');
    assert.throws(() => s.overwrite(4, 11, 'XX'), /Cannot overwrite across a split point/);
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

  it('should preserve lone-surrogate boundaries for reversed moved slices', () => {
    const s = new MagicString('ab🤷efghIJkl');
    s.move(2, 4, 8);
    s.move(8, 10, 4);

    assert.strictEqual(s.toString(), 'abIJefgh🤷kl');
    assert.strictEqual(s.slice(-3, 3), 'Jefgh\uD83E');
  });
});

describe('regex replace', () => {
  it('uses UTF-16 lastIndex for sticky regexes with emoji before the match', () => {
    const s = new MagicString('\u{1F937}a');
    const regex = /a/y;

    // JS lastIndex is in UTF-16 code units: the emoji occupies indices 0-2.
    regex.lastIndex = 2;
    s.replace(regex, 'x');

    assert.strictEqual(s.toString(), '\u{1F937}x');
    assert.strictEqual(regex.lastIndex, 3);
  });

  it('returns correct lastIndex when match ends at a supplementary character boundary', () => {
    // Matching the emoji itself — the match end byte offset lands on the
    // low surrogate entry in the mapper, which must be skipped to produce
    // the correct UTF-16 lastIndex.
    const s = new MagicString('A\u{1F937}B');
    const regex = /./uy;

    // Start at the emoji (UTF-16 index 1).
    regex.lastIndex = 1;
    s.replace(regex, 'X');

    assert.strictEqual(s.toString(), 'AXB');
    // The emoji occupies UTF-16 indices 1-2, so lastIndex should be 3.
    assert.strictEqual(regex.lastIndex, 3);
  });
});

describe('lastLine', () => {
  it('returns the content after the last newline', () => {
    assert.strictEqual(new MagicString('abc\ndef').lastLine(), 'def');
    assert.strictEqual(new MagicString('abc').lastLine(), 'abc');
    assert.strictEqual(new MagicString('abc\n').lastLine(), '');
  });

  describe('keeps fragments after the newline fragment (#10023)', () => {
    it('keeps outro fragments appended after a newline fragment', () => {
      const s = new MagicString('x');
      s.append('pre');
      s.append('a\nb');
      s.append('c');
      assert.strictEqual(s.toString(), 'xprea\nbc');
      assert.strictEqual(s.lastLine(), 'bc');
    });

    it('keeps intro fragments prepended after a newline fragment', () => {
      const s = new MagicString('x');
      s.prepend('r');
      s.prepend('p\nq');
      s.prepend('pre');
      assert.strictEqual(s.toString(), 'prep\nqrx');
      assert.strictEqual(s.lastLine(), 'qrx');
    });
  });
});

// ===========================================================================
// Ported from the plain `replace` / `replaceAll` unit tests in
// `crates/string_wizard/tests/magic_string_replace.rs`. Running them through
// the JS `RolldownMagicString` binding exercises the full pipeline — in
// particular the UTF-16 (JS string index) -> UTF-8 (byte offset) location
// conversion that only exists on the binding boundary.
// ===========================================================================

describe('string_wizard: replace', () => {
  it('works with string replace', () => {
    const s = new MagicString('1 2 1 2');
    s.replace('2', '3');
    assert.strictEqual(s.toString(), '1 3 1 2');
  });

  it('should not search back', () => {
    const s = new MagicString('122121');
    s.replace('12', '21');
    assert.strictEqual(s.toString(), '212121');
  });

  describe('replaceAll', () => {
    it('works with string replace', () => {
      const s = new MagicString('1212');
      s.replaceAll('2', '3');
      assert.strictEqual(s.toString(), '1313');
    });

    it('should not search back', () => {
      const s = new MagicString('121212');
      s.replaceAll('12', '21');
      assert.strictEqual(s.toString(), '212121');
    });
  });
});
