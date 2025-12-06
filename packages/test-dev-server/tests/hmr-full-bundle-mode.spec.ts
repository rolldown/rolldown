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

    await editFile('hmr.js', (code) =>
      code.replace(
        "const foo = 'hello'",
        "const foo = 'hello1'",
      ));

    await expect.poll(() => page.textContent('.hmr')).toBe('hello1');

    await editFile(
      'hmr.js',
      (code) => code.replace("const foo = 'hello1'", "const foo = 'hello2'"),
    );

    await expect.poll(() => page.textContent('.hmr')).toBe('hello2');

    await editFile('hmr.js', (code) =>
      code.replace(
        "const foo = 'hello2'",
        "const foo = 'hello'",
      ));
    await expect.poll(() => page.textContent('.hmr')).toBe('hello');
  });
});
