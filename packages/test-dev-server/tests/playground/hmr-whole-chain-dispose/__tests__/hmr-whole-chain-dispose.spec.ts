import { describe, expect, test } from 'vitest';
import { editFile, page, waitForBuildStable } from '~utils';

// On a hot update EVERY re-executed module's `hot.dispose(cb)` runs — the
// whole evicted chain (baz, bar, AND the boundary foo), not just the accepted
// module. `import.meta.hot.data` is PRESERVED across a module's generations
// (Vite's documented contract, https://vite.dev/guide/api-hmr#hot-data): the
// disposer receives the existing bag, and a generation's own direct writes
// into `hot.data` remain visible to the next generation.

interface HotState {
  id: string;
  gen: number | null;
  own: string | null;
}

const readDisposed = () =>
  page.evaluate(() => (window as unknown as { __disposed?: string[] }).__disposed ?? []);
const readHotStates = () =>
  page.evaluate(() => (window as unknown as { __hotStates?: HotState[] }).__hotStates ?? []);

describe('hmr-whole-chain-dispose', () => {
  test('every re-executed module disposes and carries its hot.data forward', async () => {
    // Let any spurious startup rebuild/reload settle before the un-polled
    // window reads below (a reload mid-read destroys the evaluate context).
    await waitForBuildStable();
    await expect.poll(() => page.textContent('.chain')).toBe('bar(baz-v1)');

    // Generation 0 saw empty bags: no `gen` from a previous disposer, and no
    // own-write yet (this generation records BEFORE writing it).
    await expect.poll(async () => (await readHotStates()).length).toBe(3);
    const gen0 = await readHotStates();
    for (const state of gen0) {
      expect(state.gen).toBe(null);
      expect(state.own).toBe(null);
    }

    // --- First edit: the whole chain re-executes ---------------------------
    editFile('baz.js', (code) => code.replace("'baz-v1'", "'baz-v2'"));
    await expect.poll(() => page.textContent('.chain')).toBe('bar(baz-v2)');

    // Whole-chain dispose: baz AND bar AND the boundary foo all disposed.
    const disposedOnce = await readDisposed();
    expect(disposedOnce).toHaveLength(3);
    expect(disposedOnce).toEqual(expect.arrayContaining(['foo', 'bar', 'baz']));

    // Generation 1 reads back the SAME preserved bag: the disposer's `gen: 1`,
    // and generation 0's own direct `hot.data` write survives intact.
    const gen1 = (await readHotStates()).slice(3);
    expect(gen1).toHaveLength(3);
    expect(gen1.map((s) => s.id)).toEqual(expect.arrayContaining(['foo', 'bar', 'baz']));
    for (const state of gen1) {
      expect(state.gen).toBe(1);
      expect(state.own).toBe(`${state.id}-own-write`);
    }

    await waitForBuildStable();

    // --- Second edit: same again, one generation later ----------------------
    editFile('baz.js', (code) => code.replace("'baz-v2'", "'baz-v3'"));
    await expect.poll(() => page.textContent('.chain')).toBe('bar(baz-v3)');

    const disposedTwice = await readDisposed();
    expect(disposedTwice).toHaveLength(6);

    const gen2 = (await readHotStates()).slice(6);
    expect(gen2).toHaveLength(3);
    for (const state of gen2) {
      expect(state.gen).toBe(2);
      expect(state.own).toBe(`${state.id}-own-write`);
    }

    await waitForBuildStable();
  });
});
