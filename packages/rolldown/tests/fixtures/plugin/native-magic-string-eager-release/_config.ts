import { isThreadlessWasi } from '@tests/runtime-flavor';
import type { RolldownMagicString } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

// `meta.magicString` mints a native box holding the source text plus a UTF-16
// mapping table -- together about nine times the source bytes. On the
// threadless-WASI flavor no finalizer runs, so the hook wrappers release it
// themselves once the invocation settles (src/utils/threadless-free.ts); the
// lazy flavors leave it alive for the finalizer. Asserting both contracts
// instead of skipping the lazy flavors is what proves the eager path released
// it WITHOUT breaking the hook that produced the code.
let transformMagicString: RolldownMagicString | undefined;
let renderChunkMagicString: RolldownMagicString | undefined;

// Retained WITHOUT reading `magicString` during the hook: `afterTest` reads it
// late. On the eager flavor the getter must refuse a post-settle FIRST mint
// (nothing would ever release the box); the lazy flavors keep the historical
// late mint alive for the finalizer.
let retainedTransformMeta: { magicString?: RolldownMagicString } | undefined;
let retainedRenderChunkMeta: { magicString?: RolldownMagicString } | undefined;

function expectReleasedByItsHook(label: string, box: RolldownMagicString): void {
  const first = box.dropInner();
  if (isThreadlessWasi) {
    // The hook's `finally` already released it, so nothing is left here.
    expect(first.freed, label).toBe(false);
    expect(first.reason, label).toContain('already been freed');
  } else {
    // Lazy flavors still hold the payload, so this is its first release.
    expect(first.freed, label).toBe(true);
  }
  // A repeated drop must report, never crash.
  const second = box.dropInner();
  expect(second.freed, label).toBe(false);
  expect(second.reason, label).toContain('already been freed');
  // And a released instance refuses reads rather than handing back the empty
  // `MagicString` left behind.
  expect(() => box.toString(), label).toThrow();
}

export default defineTest({
  // Module-level state shared between the hooks and `afterTest`.
  sequential: true,
  config: {
    input: ['main.js'],
    // `renderChunk` only gets a `meta.magicString` getter under this flag.
    experimental: {
      nativeMagicString: true,
    },
    output: {
      sourcemap: true,
    },
    plugins: [
      {
        name: 'test-magic-string-eager-release',
        transform(code, id, meta) {
          if (id.startsWith('\0') || !meta?.magicString) {
            return null;
          }
          transformMagicString = meta.magicString;
          transformMagicString.append('\nconsole.log("transformed");');
          // Returning it hands the string itself to `sendMagicString`, which
          // leaves the mapping table -- the bulk of the footprint -- behind.
          // `map: null` signals the map went out-of-band through that channel.
          return { code: transformMagicString, map: null };
        },
        renderChunk(_code, _chunk, _options, meta) {
          if (!meta.magicString) {
            return null;
          }
          renderChunkMagicString = meta.magicString;
          renderChunkMagicString.append('\n// rendered');
          return renderChunkMagicString;
        },
      },
      {
        // Runs after the plugin above, so the `renderChunk` meta getter left
        // installed on the shared meta object is this plugin's un-minted one.
        name: 'test-magic-string-retain-meta',
        transform(_code, id, meta) {
          if (!id.startsWith('\0')) {
            retainedTransformMeta = meta;
          }
          return null;
        },
        renderChunk(_code, _chunk, _options, meta) {
          retainedRenderChunkMeta = meta;
          return null;
        },
      },
    ],
  },
  afterTest(output) {
    // Both hooks still did their job.
    expect(output.output[0].code).toContain('transformed');
    expect(output.output[0].code).toContain('// rendered');

    expect(transformMagicString).toBeDefined();
    expectReleasedByItsHook('transform', transformMagicString!);

    expect(renderChunkMagicString).toBeDefined();
    expectReleasedByItsHook('renderChunk', renderChunkMagicString!);

    // Late FIRST access through a retained meta, after both hooks settled.
    expect(retainedTransformMeta).toBeDefined();
    expect(retainedRenderChunkMeta).toBeDefined();
    if (isThreadlessWasi) {
      // The eager flavor denies the mint with the same error surface reads of
      // an eagerly released box already use.
      expect(() => retainedTransformMeta!.magicString).toThrow(/no longer usable/);
      expect(() => retainedRenderChunkMeta!.magicString).toThrow(/no longer usable/);
    } else {
      // The lazy flavors still mint a live, usable box late; releasing it here
      // is this test being tidy, not part of the contract.
      const lateTransform = retainedTransformMeta!.magicString!;
      expect(lateTransform.original).toContain("console.log('hello')");
      expect(lateTransform.dropInner().freed).toBe(true);
      const lateRenderChunk = retainedRenderChunkMeta!.magicString!;
      expect(lateRenderChunk.original.length).toBeGreaterThan(0);
      expect(lateRenderChunk.dropInner().freed).toBe(true);
    }
  },
});
