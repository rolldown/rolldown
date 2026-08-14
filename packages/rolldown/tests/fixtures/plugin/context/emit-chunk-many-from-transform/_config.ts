// Regression test for a deadlock that triggered when a plugin emitted
// many chunks (>1024) from inside a `transform` hook.
//
// Root cause: `emit_chunk` is exposed to JS as a synchronous binding
// that internally `block_on`s a future which sends an `AddEntryModule`
// message over the module loader's `tokio::sync::mpsc::channel(1024)`.
// Once 1025 emits were queued without the loader draining any, the
// `tx.send(...).await` inside the future blocked the main JS thread.
// The loader could not drain because processing each `AddEntryModule`
// requires dispatching plugin hooks (`load`, `resolveId`) back to the
// main JS thread via TSFN — but the JS thread was blocked inside
// `block_on`. Classic producer-blocked-on-consumer-blocked-on-producer
// cycle.
//
// Fix: switch the loader's message channel to `unbounded_channel()`.
// `tx.send` becomes synchronous and infallible, so the future completes
// immediately on the same poll, `block_on` returns instantly, the JS
// thread is never actually held, and the loader drains the queued
// entries after the transform returns.
//
// EMIT_COUNT is intentionally set above the old 1024 ceiling so that
// any future regression that re-introduces a bounded channel of similar
// size will be caught here.

import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const EMIT_COUNT = 2000;
const referenceIds: string[] = [];

export default defineTest({
  sequential: true,
  config: {
    plugins: [
      {
        name: 'test-emit-chunk-many-from-transform',
        resolveId(source) {
          if (source.startsWith('virtual:emit:')) {
            return source;
          }
        },
        load(id) {
          if (id.startsWith('virtual:emit:')) {
            const n = id.slice('virtual:emit:'.length);
            return `export const id = ${JSON.stringify(n)};\n`;
          }
        },
        transform(_code, id) {
          if (!id.endsWith('main.js')) return null;
          for (let i = 0; i < EMIT_COUNT; i++) {
            const referenceId = this.emitFile({
              type: 'chunk',
              id: `virtual:emit:${i}`,
              name: `chunks/chunk-${i}`,
            });
            referenceIds.push(referenceId);
          }
          return null;
        },
      },
    ],
  },
  afterTest: () => {
    expect(referenceIds.length).toBe(EMIT_COUNT);
  },
});
