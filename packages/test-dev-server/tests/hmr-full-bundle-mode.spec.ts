import { setTimeout } from 'node:timers/promises';
import { describe, expect, test } from 'vitest';
import { editFile, getPage } from './test-utils';

describe('hmr-full-bundle-mode', () => {
  test.sequential('should render initial content', async () => {
    const page = getPage();

    const headingText = await page.textContent('h1');
    expect(headingText).toBe('HMR Full Bundle Mode');

    const appText = await page.textContent('.app');
    expect(appText).toBe('hello');

    const hmrText = await page.textContent('.hmr');
    expect(hmrText).toBe('hello');
  });

  test.sequential('basic HMR', async () => {
    const page = getPage();

    editFile('hmr.js', (code) =>
      code.replace(
        "const foo = 'hello'",
        "const foo = 'hello1'",
      ));

    await expect.poll(() => page.textContent('.hmr')).toBe('hello1');

    editFile(
      'hmr.js',
      (code) => code.replace("const foo = 'hello1'", "const foo = 'hello2'"),
    );

    await expect.poll(() => page.textContent('.hmr')).toBe('hello2');

    await setTimeout(1000);

    editFile('hmr.js', (code) =>
      code.replace(
        "const foo = 'hello2'",
        "const foo = 'hello'",
      ));
    await expect.poll(() => page.textContent('.hmr')).toBe('hello');
  });

  // https://github.com/vitejs/rolldown-vite/blob/942cb2b51b59fd6aefe886ec78eb34fff56ead34/playground/hmr-full-bundle-mode/__tests__/hmr-full-bundle-mode.spec.ts#L49-L70
  test.sequential('debounce bundle', async () => {
    const page = getPage();
    editFile('main.js', (code) =>
      code.replace(
        "text('.app', 'hello')",
        "text('.app', 'hello1')\n" + '// @delay-transform',
      ));
    await setTimeout(100);
    editFile(
      'main.js',
      (code) =>
        code.replace("text('.app', 'hello1')", "text('.app', 'hello2')"),
    );
    await expect.poll(() => page.textContent('.app')).toBe('hello2');

    editFile('main.js', (code) =>
      code.replace(
        "text('.app', 'hello2')\n" + '// @delay-transform',
        "text('.app', 'hello')",
      ));
    await expect.poll(() => page.textContent('.app')).toBe('hello');
  });

  // https://github.com/vitejs/rolldown-vite/blob/942cb2b51b59fd6aefe886ec78eb34fff56ead34/playground/hmr-full-bundle-mode/__tests__/hmr-full-bundle-mode.spec.ts#L101-L123
  test.sequential('continuous generate hmr patch', async () => {
    const page = getPage();

    editFile('hmr.js', (code) =>
      code.replace(
        "const foo = 'hello'",
        "const foo = 'hello1'\n" + '// @delay-transform',
      ));
    await setTimeout(100);
    editFile(
      'hmr.js',
      (code) => code.replace("const foo = 'hello1'", "const foo = 'hello2'"),
    );
    await expect.poll(() => page.textContent('.hmr')).toBe('hello2');

    editFile('hmr.js', (code) =>
      code.replace(
        "const foo = 'hello2'\n" + '// @delay-transform',
        "const foo = 'hello'",
      ));
    await expect.poll(() => page.textContent('.hmr')).toBe('hello');
  });
});
