// Regression test for a deadlock that triggered when many input modules
// each emitted a single chunk from their own `transform` hook.
//
// This is the "realistic plugin" variant of
// `emit-chunk-many-from-transform`. Instead of one hook emitting
// thousands of chunks in a tight loop, here N virtual input files each
// emit exactly ONE chunk from their transform — the pattern a normal
// plugin (e.g. an RSC client-component collector) produces.
//
// Root cause (same as the sibling fixture, different trigger):
// `emit_chunk` is exposed to JS as a synchronous napi binding that
// `block_on`s a future which sends `AddEntryModule` over the module
// loader's bounded `mpsc::channel(1024)`. Under parallelism the loader
// cannot drain fast enough: as it pops `AddEntryModule` messages it
// spawns new module tasks whose own `resolveId`/`load`/`transform`
// hooks dispatch back to the JS thread via TSFN — but the JS thread is
// blocked inside `block_on`, so those callbacks never run, the spawned
// tasks never complete, and the channel stays at capacity. Any further
// `emit_chunk` call blocks the producer transform forever.
//
// Important: this deadlock can trigger at an emit count WELL below the
// 1024 channel capacity. In local testing, 600 per-file emits hung
// after processing ~400 transforms. We use 1500 here to give a
// comfortable margin above any scheduler-noise-dependent threshold, so
// any regression that re-introduces a bounded channel at similar
// capacity is caught deterministically.
//
// Fix: the same one as `emit-chunk-many-from-transform` — switch the
// loader's message channel to `unbounded_channel()`. `tx.send` becomes
// sync and infallible, `block_on` never actually parks, the JS thread
// is released immediately after each emit, and the TSFN callbacks the
// loader is waiting on are free to run.

import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const EMIT_COUNT = 1500;

const inputs: Record<string, string> = {};
for (let i = 0; i < EMIT_COUNT; i++) {
  inputs[`entry-${i}`] = `\0virtual:input:${i}`;
}

let transformCount = 0;

export default defineTest({
  sequential: true,
  config: {
    input: inputs,
    plugins: [
      {
        name: 'test-emit-chunk-many-parallel-inputs',
        resolveId(source) {
          if (source.startsWith('\0virtual:input:')) return source;
          if (source.startsWith('\0virtual:side:')) return source;
        },
        load(id) {
          if (id.startsWith('\0virtual:input:')) {
            const n = id.slice('\0virtual:input:'.length);
            return `export const id = ${JSON.stringify(n)};\n`;
          }
          if (id.startsWith('\0virtual:side:')) {
            const n = id.slice('\0virtual:side:'.length);
            return `export const side = ${JSON.stringify(n)};\n`;
          }
        },
        transform(_code, id) {
          if (!id.startsWith('\0virtual:input:')) return null;
          const n = id.slice('\0virtual:input:'.length);
          this.emitFile({
            type: 'chunk',
            id: `\0virtual:side:${n}`,
            name: `sides/side-${n}`,
          });
          transformCount++;
          return null;
        },
      },
    ],
  },
  afterTest: () => {
    expect(transformCount).toBe(EMIT_COUNT);
  },
});
