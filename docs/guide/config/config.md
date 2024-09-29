# Configuration Files

It is recommended to utilize a configuration file rather than directly invoking the CLI to access Rolldown during development. You can create a `rolldown.config.js` file within the root directory of your project.

```js [rolldown.config.js]
export default {
  input: 'src/main.ts',
  output: {
    format: 'iife',
  },
}
```

At present, our support is limited to JavaScript files. TypeScript and JSON configuration files are not supported. If you are using `.mjs` or `.cjs` files, please specify the file name at the command line interface.

It is more recommended to utilize the `defineConfig` method to support type annotation and intellisense.

```js [rolldown.config.js]
import { defineConfig } from 'rolldown'

export default defineConfig({
  input: 'src/main.ts',
  output: {
    format: 'iife',
  },
})
```

You can also specify multiple configurations as an array, and Rolldown will bundle them in parallel.

```js [rolldown.config.js]
import { defineConfig } from 'rolldown'

export default defineConfig([
  {
    input: 'src/main.ts',
    output: {
      format: 'esm',
    },
  },
  {
    input: 'src/worker.ts',
    output: {
      format: 'iife',
      dir: 'dist/worker',
    },
  },
])
```

## Configuration Options

_We’re planning to use JSON Schema to automatically generate the configuration options here in the future. Stay tuned!_

For the time being, you can refer to the `/packages/rolldown/src/options` folder for the available options (utilizing the `zod` schema).
