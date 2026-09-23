// Adapted from https://github.com/rollup/rollup/blob/3b560f7c889a63968dabc9b6970aabf52a77d3fd/src/utils/asyncFlatten.ts

import { hasCallableThenWithoutInvokingAccessor } from './prototype-chain';

export type SynchronousCallbackRunner = <T>(callback: () => T) => T;

const runDirectly: SynchronousCallbackRunner = (callback) => callback();

// Values assimilate the way the callback-settlement path (async-context.ts,
// `settleThenable`) already does, so a plugin option and a callback result
// follow one Promise Resolution Procedure:
// - `then` is read once per value per resolution step, inside the callback scope;
// - the captured `then` is called in a later promise job that re-enters the
//   scope, because the browser scope does not propagate through jobs;
// - a callable `then` wins over `Array.isArray` at every level;
// - a resolved value is classified while the resolving function runs, before a
//   later job can mutate it.
// See internal-docs/async-context/implementation.md.
interface BoxedValue<T> {
  arrayChain: Set<unknown[]> | undefined;
  thenableChain: Set<object> | undefined;
  // Never named `then`: boxes travel through native promises and through
  // `CloseCallbackScope.run`, which both assimilate a thenable result.
  thenMethod: Function | undefined;
  value: T;
}

export async function asyncFlatten<T>(
  array: T[],
  runSynchronousCallback: SynchronousCallbackRunner = runDirectly,
): Promise<T[]> {
  // Scope callbacks assign instead of returning: `CloseCallbackScope.run`
  // reads `then` on whatever a callback returns.
  let pending!: BoxedValue<T>[];
  runSynchronousCallback(() => {
    pending = array.flatMap((value) => flattenArrays(classify(value, undefined, undefined)));
  });
  while (pending.some((box) => box.thenMethod !== undefined)) {
    const boxed = await Promise.all(
      pending.map((box) =>
        box.thenMethod === undefined
          ? Promise.resolve(box)
          : assimilateThenable(box, box.thenMethod, runSynchronousCallback),
      ),
    );
    // Array traps are user callbacks that native close can end up waiting on,
    // so flattening runs inside the close-callback scope.
    runSynchronousCallback(() => {
      pending = boxed.flatMap(flattenArrays);
    });
  }
  return pending.map(({ value }) => value);
}

/** The one `then` read for this resolution step. Runs inside the callback scope. */
function classify<T>(
  value: T,
  arrayChain: Set<unknown[]> | undefined,
  thenableChain: Set<object> | undefined,
): BoxedValue<T> {
  const box: BoxedValue<T> = { arrayChain, thenableChain, thenMethod: undefined, value };
  if ((typeof value !== 'object' || value === null) && typeof value !== 'function') return box;
  if (thenableChain?.has(value)) {
    // A repeated value is only a cycle while it is STILL thenable: the spec
    // re-reads `then` on every resolution step, so a thenable that shed its
    // `then` before resolving to itself is terminal. The descriptor walk keeps
    // an accessor from running a second time.
    if (hasCallableThenWithoutInvokingAccessor(value)) {
      throw new TypeError('Thenable cycle detected while flattening values');
    }
    return box;
  }
  const then = Reflect.get(value, 'then');
  if (typeof then === 'function') box.thenMethod = then;
  return box;
}

function assimilateThenable<T>(
  { arrayChain, thenableChain, value }: BoxedValue<T>,
  then: Function,
  runSynchronousCallback: SynchronousCallbackRunner,
): Promise<BoxedValue<T>> {
  const nextThenableChain = new Set(thenableChain);
  nextThenableChain.add(value as object);
  return new Promise<BoxedValue<T>>((resolve, reject) => {
    // PromiseResolveThenableJob: call the captured `then` in a later job; the
    // first resolving function to run wins, and a throw after it is ignored.
    void Promise.resolve().then(() => {
      let settled = false;
      const resolveOnce = (resolved: T) => {
        if (settled) return;
        settled = true;
        let next!: BoxedValue<T>;
        try {
          runSynchronousCallback(() => {
            next = classify(resolved, arrayChain, nextThenableChain);
          });
        } catch (error) {
          reject(error);
          return;
        }
        resolve(next);
      };
      const rejectOnce = (reason?: unknown) => {
        if (settled) return;
        settled = true;
        reject(reason);
      };
      try {
        runSynchronousCallback(() => {
          Reflect.apply(then, value, [resolveOnce, rejectOnce]);
        });
      } catch (error) {
        rejectOnce(error);
      }
    });
  });
}

function flattenArrays<T>(boxed: BoxedValue<T>): BoxedValue<T>[] {
  const flattened: BoxedValue<T>[] = [];
  const pending: FlattenEntry[] = [{ boxed, kind: 'value' }];
  while (pending.length > 0) {
    const entry = pending.pop()!;
    if (entry.kind === 'array') {
      while (entry.index < entry.length) {
        const index = entry.index;
        entry.index += 1;
        if (!(index in entry.value)) continue;
        pending.push(entry, {
          boxed: classify(entry.value[index], entry.arrayChain, entry.thenableChain),
          kind: 'value',
        });
        break;
      }
      continue;
    }

    const current = entry.boxed;
    // A thenable array waits for assimilation instead of being spread.
    if (current.thenMethod !== undefined || !Array.isArray(current.value)) {
      flattened.push(current as BoxedValue<T>);
      continue;
    }
    if (current.arrayChain?.has(current.value)) {
      throw new TypeError('Array cycle detected while flattening values');
    }

    const nextArrayChain = new Set(current.arrayChain);
    nextArrayChain.add(current.value);
    pending.push({
      arrayChain: nextArrayChain,
      index: 0,
      kind: 'array',
      length: current.value.length,
      thenableChain: current.thenableChain,
      value: current.value,
    });
  }
  return flattened;
}

type FlattenEntry =
  | {
      boxed: BoxedValue<unknown>;
      kind: 'value';
    }
  | {
      arrayChain: Set<unknown[]>;
      index: number;
      kind: 'array';
      length: number;
      thenableChain: Set<object> | undefined;
      value: unknown[];
    };
