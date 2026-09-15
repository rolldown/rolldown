import { describe, expect, test } from 'vitest';
import { editFile, page, waitForBuildStable } from '~utils';

// Accept-syntax matrix: each accept shape (self / dep / array), written both plain
// (`import.meta.hot.accept`) and optional-chained (`import.meta.hot?.accept`). The compiler
// must recognize both — the accepted-dep specifier is rewritten to the resolved module id
// either way; if the `?.` form is missed the specifier stays raw and the client full-reloads.
// Every edit must hot-update in place: `window.__marker` survives (a full reload wipes it).

/** Plant a marker on `window`; any full page reload wipes it. */
const plantMarker = () =>
  page.evaluate(() => ((window as unknown as { __marker?: string }).__marker = 'alive'));
const readMarker = () =>
  page.evaluate(() => (window as unknown as { __marker?: string }).__marker ?? null);

/** Edit `<file>` swapping `<from>`→`<to>`, assert `<sel>` hot-updates and no reload happened. */
async function expectHotUpdate(file: string, sel: string, from: string, to: string) {
  await waitForBuildStable();
  await plantMarker();

  editFile(file, (code) => code.replace(from, to));
  await expect.poll(() => page.textContent(sel)).toBe(to.replaceAll("'", ''));

  expect(await readMarker()).toBe('alive');
  await waitForBuildStable();
}

describe('hmr-accept-syntax', () => {
  test('renders the initial values', async () => {
    await waitForBuildStable();
    for (const [sel, v] of [
      ['.self-plain', 'self-plain-v1'],
      ['.self-optional', 'self-optional-v1'],
      ['.dep-plain', 'dep-plain-v1'],
      ['.dep-optional', 'dep-optional-v1'],
      ['.arr-plain', 'arr-plain-v1'],
      ['.arr-optional', 'arr-optional-v1'],
    ]) {
      await expect.poll(() => page.textContent(sel)).toBe(v);
    }
  });

  // --- self-accept: import.meta.hot[?].accept(cb) ---
  test('plain self-accept `import.meta.hot.accept(cb)`', async () => {
    await expectHotUpdate('self-plain.js', '.self-plain', "'self-plain-v1'", "'self-plain-v2'");
  });
  test('optional self-accept `import.meta.hot?.accept(cb)`', async () => {
    await expectHotUpdate('self-optional.js', '.self-optional', "'self-optional-v1'", "'self-optional-v2'");
  });

  // --- accept-dep: import.meta.hot[?].accept('./dep', cb) ---
  test('plain accept-dep `import.meta.hot.accept("./dep", cb)`', async () => {
    await expectHotUpdate('dep-plain-target.js', '.dep-plain', "'dep-plain-v1'", "'dep-plain-v2'");
  });
  test('optional accept-dep `import.meta.hot?.accept("./dep", cb)`', async () => {
    await expectHotUpdate('dep-optional-target.js', '.dep-optional', "'dep-optional-v1'", "'dep-optional-v2'");
  });

  // --- array accept-dep: import.meta.hot[?].accept(['./dep'], cb) ---
  test('plain array accept-dep `import.meta.hot.accept(["./dep"], cb)`', async () => {
    await expectHotUpdate('arr-plain-target.js', '.arr-plain', "'arr-plain-v1'", "'arr-plain-v2'");
  });
  test('optional array accept-dep `import.meta.hot?.accept(["./dep"], cb)`', async () => {
    await expectHotUpdate('arr-optional-target.js', '.arr-optional', "'arr-optional-v1'", "'arr-optional-v2'");
  });
});
