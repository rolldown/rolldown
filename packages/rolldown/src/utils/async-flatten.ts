// Adapted from https://github.com/rollup/rollup/blob/3b560f7c889a63968dabc9b6970aabf52a77d3fd/src/utils/asyncFlatten.ts

export type SynchronousCallbackRunner = <T>(callback: () => T) => T;

const runDirectly: SynchronousCallbackRunner = (callback) => callback();

interface BoxedValue<T> {
  arrayChain: Set<unknown[]> | undefined;
  thenableChain: Set<object> | undefined;
  value: T;
}

export async function asyncFlatten<T>(
  array: T[],
  runSynchronousCallback: SynchronousCallbackRunner = runDirectly,
): Promise<T[]> {
  let pending = array.map(
    (value): BoxedValue<T> => ({
      arrayChain: undefined,
      thenableChain: undefined,
      value,
    }),
  );
  while (true) {
    let requiresAnotherPass = false;
    const boxed = await Promise.all(
      pending.map(({ arrayChain, thenableChain, value }) => {
        if (Array.isArray(value)) {
          requiresAnotherPass = true;
          return Promise.resolve({ arrayChain, thenableChain, value });
        }
        return Promise.resolve(
          assimilateThenable(value, arrayChain, thenableChain, runSynchronousCallback, () => {
            requiresAnotherPass = true;
          }),
        );
      }),
    );
    pending = boxed.flatMap(flattenArrays);
    if (!requiresAnotherPass) return pending.map(({ value }) => value);
  }
}

function assimilateThenable<T>(
  value: T,
  arrayChain: Set<unknown[]> | undefined,
  thenableChain: Set<object> | undefined,
  runSynchronousCallback: SynchronousCallbackRunner,
  markAssimilated: () => void,
): BoxedValue<T> | Promise<BoxedValue<T>> {
  return runSynchronousCallback(() => {
    if ((typeof value !== 'object' || value === null) && typeof value !== 'function') {
      return { arrayChain, thenableChain, value };
    }
    if (thenableChain?.has(value)) {
      throw new TypeError('Thenable cycle detected while flattening values');
    }

    const then = Reflect.get(value, 'then');
    if (typeof then !== 'function') return { arrayChain, thenableChain, value };

    markAssimilated();
    const nextThenableChain = new Set(thenableChain);
    nextThenableChain.add(value);
    return new Promise<BoxedValue<T>>((resolve, reject) => {
      Reflect.apply(then, value, [
        (resolved: T) => resolve({ arrayChain, thenableChain: nextThenableChain, value: resolved }),
        reject,
      ]);
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
          boxed: {
            arrayChain: entry.arrayChain,
            thenableChain: entry.thenableChain,
            value: entry.value[index],
          },
          kind: 'value',
        });
        break;
      }
      continue;
    }

    const current = entry.boxed;
    if (!Array.isArray(current.value)) {
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
