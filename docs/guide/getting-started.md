# Getting Started

:::warning 🚧 Beta Software
Rolldown is currently in beta status. While it can already handle most production use cases, there may still be bugs and rough edges. Most notably, the built-in minification feature is still in early work-in-progress status.
:::

## Installation

::: code-group

```sh [npm]
$ npm install -D rolldown
```

```sh [pnpm]
$ pnpm add -D rolldown
```

```sh [yarn]
$ yarn add -D rolldown
```

```sh [bun]
$ bun add -D rolldown
```

:::

### Release Channels

- [latest](https://www.npmjs.com/package/rolldown?activeTab=versions): currently `1.0.0-beta.*`.
- [pkg.pr.new](https://pkg.pr.new/~/rolldown/rolldown): continuously released from the `main` branch. Install with `npm i https://pkg.pr.new/rolldown@sha` where `sha` is a successful build listed on [pkg.pr.new](https://pkg.pr.new/~/rolldown/rolldown).

## Using the CLI

To verify Rolldown is installed correctly, run the following in the directory where you installed it:

```sh
$ ./node_modules/.bin/rolldown --version
```

You can also check out the CLI options and examples with:

```sh
$ ./node_modules/.bin/rolldown --help
```

### Your first bundle

Let's create two source JavaScript files:

```js [src/main.js]
import { hello } from './hello.js';

hello();
```

```js [src/hello.js]
export function hello() {
  console.log('Hello Rolldown!');
}
```

Then run the following in the command line:

```sh
$ ./node_modules/.bin/rolldown src/main.js --file bundle.js
```

You should see the content written to `bundle.js` in your current directory. Let's run it to verify it's working:

```sh
$ node bundle.js
```

You should see `Hello Rolldown!` printed.

### Using the CLI in npm scripts

To avoid typing the long command, we can move it inside an npm script:

```json{5} [package.json]
{
  "name": "my-rolldown-project",
  "type": "module",
  "scripts": {
    "build": "rolldown src/main.js --file bundle.js"
  },
  "devDependencies": {
    "rolldown": "^1.0.0-beta.1"
  }
}
```

Now we can run the build with just:

```sh
$ npm run build
```

## Using the Config File

When more options are needed, it is recommended to use a config file for more flexibility. Let's create the following config file:

```js [rolldown.config.js]
import { defineConfig } from 'rolldown';

export default defineConfig({
  input: 'src/main.js',
  output: {
    file: 'bundle.js',
  },
});
```

Rolldown supports most of the [Rollup config options](https://rollupjs.org/configuration-options), with some [notable additional features](./features.md).

While exporting a plain object also works, it is recommended to utilize the `defineConfig` helper method to get options intellisense and auto-completion. This helper is provided purely for the types and returns the options as-is.

Next, in the npm script, we can instruct Rolldown to use the config file with the `--config` CLI option (`-c` for short):

```json{5} [package.json]
{
  "name": "my-rolldown-project",
  "type": "module",
  "scripts": {
    "build": "rolldown -c"
  },
  "devDependencies": {
    "rolldown": "^1.0.0-beta.1"
  }
}
```

### TypeScript config file

TypeScript config file is also supported out of the box:

```json{5} [package.json]
{
  "name": "my-rolldown-project",
  "type": "module",
  "scripts": {
    "build": "rolldown -c rolldown.config.ts"
  },
  "devDependencies": {
    "rolldown": "^1.0.0-beta.1"
  }
}
```

```js [rolldown.config.ts]
import { defineConfig } from 'rolldown';

export default defineConfig({
  input: 'src/main.js',
  output: {
    file: 'bundle.js',
  },
});
```

:::warning Specifying config file name
The default config file used with the `-c` flag is `rolldown.config.js`. If you are using `.ts` or `.mjs` extensions, make sure to specify the full filename with e.g. `rolldown -c rolldown.config.ts`.
:::

### Multiple builds in the same config

You can also specify multiple configurations as an array, and Rolldown will bundle them in parallel.

```js [rolldown.config.js]
import { defineConfig } from 'rolldown';

export default defineConfig([
  {
    input: 'src/main.js',
    output: {
      format: 'esm',
    },
  },
  {
    input: 'src/worker.js',
    output: {
      format: 'iife',
      dir: 'dist/worker',
    },
  },
]);
```

## Using Plugins

Rolldown's plugin API is identical to that of Rollup's, so you can reuse most of the existing Rollup plugins when using Rolldown. That said, Rolldown provides many [built-in features](./features.md) that make it unnecessary to use plugins.

## Using the API

Rolldown provides a JavaScript API that is compatible with [Rollup's](https://rollupjs.org/javascript-api/), which separates `input` and `output` options:

```js
import { rolldown } from 'rolldown';

const bundle = await rolldown({
  // input options
  input: 'src/main.js',
});

// generate bundles in memory with different output options
await bundle.generate({
  // output options
  format: 'esm',
});
await bundle.generate({
  // output options
  format: 'cjs',
});

// or directly write to disk
await bundle.write({
  file: 'bundle.js',
});
```

Alternatively, you can also use the more concise `build` API, which accepts the exact same options as the config file export:

```js
import { build } from 'rolldown';

// build writes to disk by default
await build({
  input: 'src/main.js',
  output: {
    file: 'bundle.js',
  },
});
```

## Using the Watcher

The rolldown watcher api is compatible with rollup [watch](https://rollupjs.org/javascript-api/#rollup-watch).

```js
import { watch } from 'rolldown';

const watcher = watch({
  /* option */
}); // or watch([/* multiply option */] )

watcher.on('event', () => {});

await watcher.close(); // Here is different with rollup, the rolldown returned the promise at here.
```
