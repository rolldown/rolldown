This option is particularly useful when the output directory structure may change. This can happen when third-party modules are not marked [`external`](/reference/InputOptions.external), or while developing in a monorepo of multiple packages that rely on one another and are not marked [`external`](/reference/InputOptions.external).

#### Examples

```js
import { defineConfig } from 'rolldown';

export default defineConfig({
  input: ['src/module.js', `src/another/module.js`],
  output: {
    dir: 'dist',
    preserveModules: true,
    preserveModulesRoot: 'src',
  },
});
```

This setting ensures that the input modules will be output to the paths `dist/module.js` and `dist/another/module.js`.
