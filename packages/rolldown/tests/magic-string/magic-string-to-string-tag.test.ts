import assert from 'node:assert';
import { RolldownMagicString as MagicString } from 'rolldown';
import { describe, it } from 'vitest';

describe('MagicString isRolldownMagicString', () => {
  it('should have isRolldownMagicString on prototype', () => {
    assert.strictEqual(MagicString.prototype.isRolldownMagicString, true);
  });

  it('should be accessible on instances', () => {
    const s = new MagicString('hello');
    assert.strictEqual(s.isRolldownMagicString, true);
  });

  it('should allow detection without importing rolldown', () => {
    const s = new MagicString('hello');
    // This is the primary use case: external packages can detect native
    // BindingMagicString instances using isRolldownMagicString instead of
    // the fragile `s.constructor.name === 'RolldownMagicString'`
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
