import { describe, expect, test } from 'vitest';
import { editFile, page, waitForBuildStable } from '~utils';

// On a hot update EVERY re-executed module's `hot.dispose(cb)` runs — the
// whole evicted chain (baz, bar, AND the boundary foo), not just the accepted
// module. Each disposer receives a FRESH data bag; the next generation reads
// it via `hot.data`, and a generation's own direct writes into `hot.data` do
// NOT leak into the next generation's bag.

interface HotState {
  id: string;
  gen: number | null;
  leak: string | null;
}

const readDisposed = () =>
  page.evaluate(() => (window as unknown as { __disposed?: string[] }).__disposed ?? []);
const readHotStates = () =>
  page.evaluate(() => (window as unknown as { __hotStates?: HotState[] }).__hotStates ?? []);

describe('hmr-whole-chain-dispose', () => {
  test('every re-executed module disposes, each generation gets a fresh data bag', async () => {
    // Let any spurious startup rebuild/reload settle before the un-polled
    // window reads below (a reload mid-read destroys the evaluate context).
    await waitForBuildStable();
    await expect.poll(() => page.textContent('.chain')).toBe('bar(baz-v1)');

    // Generation 0 saw empty bags: no `gen` from a previous disposer, no leak.
    await expect.poll(async () => (await readHotStates()).length).toBe(3);
    const gen0 = await readHotStates();
    for (const state of gen0) {
      expect(state.gen).toBe(null);
      expect(state.leak).toBe(null);
    }

    // --- First edit: the whole chain re-executes ---------------------------
    editFile('baz.js', (code) => code.replace("'baz-v1'", "'baz-v2'"));
    await expect.poll(() => page.textContent('.chain')).toBe('bar(baz-v2)');

    // Whole-chain dispose: baz AND bar AND the boundary foo all disposed.
    const disposedOnce = await readDisposed();
    expect(disposedOnce).toHaveLength(3);
    expect(disposedOnce).toEqual(expect.arrayContaining(['foo', 'bar', 'baz']));

    // Generation 1 reads what generation 0's disposers wrote (`gen: 1`), and
    // does NOT see generation 0's direct `hot.data.leak` writes — the bag is
    // fresh, only the disposer's writes travel forward.
    const gen1 = (await readHotStates()).slice(3);
    expect(gen1).toHaveLength(3);
    expect(gen1.map((s) => s.id)).toEqual(expect.arrayContaining(['foo', 'bar', 'baz']));
    for (const state of gen1) {
      expect(state.gen).toBe(1);
      expect(state.leak).toBe(null);
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
      expect(state.leak).toBe(null);
    }

    await waitForBuildStable();
  });
});
