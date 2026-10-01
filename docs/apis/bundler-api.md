# Bundler API

Rolldown provides three main API functions for bundling your code programmatically.

## `rolldown(input)`

- `input` {InputOptions} The input options object.
- Returns: {Promise<RolldownBuild>} A Promise that resolves to a bundle object.

`rolldown()` is the API compatible with Rollup's `rollup` function.

```js
import { rolldown } from 'rolldown';

let bundle,
  failed = false;
try {
  bundle = await rolldown({
    input: 'src/main.js',
  });
  await bundle.write({
    format: 'esm',
  });
} catch (e) {
  console.error(e);
  failed = true;
}
if (bundle) {
  await bundle.close();
}
process.exitCode = failed ? 1 : 0;
```

See [its reference](/reference/Function.rolldown) for more details.

## `watch(input)`

- `input` {WatchOptions | WatchOptions[]} The watch options object or the list of them.
- Returns: {RolldownWatcher} A watcher object.

`watch()` is the API compatible with Rollup's `watch` function.

```js
import { watch } from 'rolldown';

const watcher = watch({/* ... */});
watcher.on('event', (event) => {
  if (event.code === 'BUNDLE_END') {
    console.log(event.duration);
    event.result.close();
  }
});

// Stop watching
watcher.close();
```

See [its reference](/reference/Function.watch) for more details.

## `build(options)`

> Stability: 1 - Experimental. This API may change in patch releases.

- `options` {BuildOptions | BuildOptions[]} The build options, or a list of them to build sequentially.
- Returns: {Promise<RolldownOutput | RolldownOutput[]>} A Promise that resolves to the build output, or the outputs for each option.

`build()` is the simplest option for most use cases. The API is similar to esbuild's `build` function. It bundles and writes in a single call with automatic cleanup.

```js
import { build } from 'rolldown';

const result = await build({
  input: 'src/main.js',
  output: {
    file: 'bundle.js',
  },
});
console.log(result);
```

See [its reference](/reference/Function.build) for more details.
