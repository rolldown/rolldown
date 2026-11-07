import { isThreadlessWasi } from '@tests/runtime-flavor';
import { parseSync } from 'rolldown/utils';
import { expect, test } from 'vitest';

// A threadless WASI host may never run GC finalizers, so `parseSync` reads every
// native `ParseResult` getter at parse time. Each getter `mem::take`s its field,
// so a second native read comes back empty: the native copy is already freed.
test.runIf(isThreadlessWasi)(
  'threadless WASI frees the native parse result at parse time',
  async () => {
    // Through vitest's module runner: the instance `rolldown/utils` uses.
    const binding: any = await import('../dist/rolldown-binding.wasip1.cjs' as string);
    const proto = (binding.ParseResult ?? binding.default.ParseResult).prototype;
    const fields = ['comments', 'errors', 'module', 'program'];
    const originals = new Map(
      fields.map((field) => [field, Object.getOwnPropertyDescriptor(proto, field)!]),
    );
    const reads: string[] = [];
    const natives = new Set<unknown>();
    for (const [field, descriptor] of originals) {
      Object.defineProperty(proto, field, {
        ...descriptor,
        get() {
          natives.add(this);
          reads.push(field);
          return descriptor.get!.call(this);
        },
      });
    }
    try {
      const result = parseSync('entry.js', 'export const a = 1; // note');
      expect(reads.toSorted()).toEqual(fields);
      expect(result.comments).toHaveLength(1);
      expect(result.program.body).toHaveLength(1);
      expect(reads).toHaveLength(fields.length);
      expect(natives.size).toBe(1);
      const [native] = natives;
      const readAgain = (field: string) => originals.get(field)!.get!.call(native);
      expect(readAgain('program')).toBe('');
      expect(readAgain('comments')).toEqual([]);
      expect(readAgain('module').staticExports).toEqual([]);
    } finally {
      for (const [field, descriptor] of originals) Object.defineProperty(proto, field, descriptor);
    }
  },
);
