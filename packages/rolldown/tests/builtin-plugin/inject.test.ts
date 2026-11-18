import { injectPlugin } from 'rolldown/plugins';
import { expect, test } from 'vitest';
import type { BindingCallableBuiltinPlugin } from '../../src/binding.cjs';

test('can be called as a transform hook', async () => {
  const plugin = injectPlugin(
    { useKey: ['#app', 'useKey'] },
    { exclude: /node_modules/ },
  ) as ReturnType<typeof injectPlugin> & BindingCallableBuiltinPlugin;
  const code = 'export const key = useKey();';

  const injected = await plugin.transform(code, '/src/main.js', { moduleType: 'js' });
  expect(injected?.code).toContain('import { useKey } from "#app";');

  const excluded = await plugin.transform(code, '/node_modules/dep/index.js', { moduleType: 'js' });
  expect(excluded).toBeNull();
});
