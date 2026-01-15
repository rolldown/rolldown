# Command Line Interface

Rolldown can be used from the command line. You can provide an optional Rolldown configuration file to simplify command line usage and enable advanced Rolldown functionality.

## Configuration Files

Rolldown configuration files are optional, but they are powerful and convenient and thus **recommended**.
A config file is an ES module that exports a default object with the desired options.
Typically, it is called `rolldown.config.js` and sits in the root directory of your project.
You can also use CJS syntax in CJS files, which uses `module.exports` instead of `export default`.
Rolldown also natively supports TypeScript configuration files.

Consult the [reference](/reference/) for a comprehensive list of options you can include in your config file.

```js [rolldown.config.js]
export default {
  input: 'src/main.js',
  output: {
    file: 'bundle.js',
    format: 'cjs',
  },
};
```

To use a config file with Rolldown, pass the `-c` (or `--config`) flag:

```shell
rolldown -c                 # use rolldown.config.{js,mjs,cjs,ts,mts,cts}
rolldown --config           # same as above
rolldown -c my.config.js    # use a custom config file
```

If you don't pass a file name, Rolldown will try to load `rolldown.config.{js,mjs,cjs,ts,mts,cts}` in the working directory.
If no config file is found, Rolldown will show an error.

You can also export a function from your config file. The function will be called with command line arguments so you can dynamically adapt your configuration:

```js [rolldown.config.js]
import { defineConfig } from 'rolldown';

export default defineConfig((commandLineArgs) => {
  if (commandLineArgs.watch) {
    // watch-specific config
  }
  return {
    input: 'src/main.js',
  };
});
```

### Config Intellisense

Since Rolldown ships with TypeScript typings, you can leverage your IDE's intellisense with JSDoc type hints:

```js [rolldown.config.js]
/** @type {import('rolldown').RolldownOptions} */
export default {
  // ...
};
```

Alternatively you can use the `defineConfig` helper, which provides intellisense without the need for JSDoc annotations:

```js [rolldown.config.js]
import { defineConfig } from 'rolldown';

export default defineConfig({
  // ...
});
```

### Configuration Arrays

To build different bundles from different inputs, you can supply an array of configuration objects:

```js [rolldown.config.js]
import { defineConfig } from 'rolldown';

export default defineConfig([
  {
    input: 'src/main.js',
    output: { format: 'esm', entryFileNames: 'bundle.esm.js' },
  },
  {
    input: 'src/main.js',
    output: { format: 'cjs', entryFileNames: 'bundle.cjs.js' },
  },
]);
```

::: tip Different outputs with same inputs

You can also supply an array for the `output` option to generate multiple outputs from the same input:

```js [rolldown.config.js]
import { defineConfig } from 'rolldown';

export default defineConfig({
  input: 'src/main.js',
  output: [
    { format: 'esm', entryFileNames: 'bundle.esm.js' },
    { format: 'cjs', entryFileNames: 'bundle.cjs.js' },
  ],
});
```

:::

## Command Line Flags

Flags can be passed as `--foo`, `--foo <value>`, or `--foo=<value>`. Boolean flags like `--minify` don't need a value, while key-value options like `--define` use comma-separated syntax: `--define key=value,key2=value2`. Many flags have short aliases (e.g., `-m` for `--minify`, `-f` for `--format`).

::: info Integration into other tools

Note that your shell interprets arguments before Rolldown sees them—quotes and wildcards may behave unexpectedly. For advanced build processes or integration into other tools, consider using the [JavaScript API](/apis/bundler-api) instead. Key differences when switching from config files to the API:

- Configuration must be an object (not a Promise or function)
- Run [`rolldown.rolldown`](/reference/Function.rolldown) separately for each set of `inputOptions` (no config arrays)
- Use [`bundle.generate(outputOptions)`](/reference/Interface.RolldownBuild#generate) or [`bundle.write(outputOptions)`](/reference/Interface.RolldownBuild#write) instead of the `output` option

:::

Many options have command line flag equivalents.
See the [reference](/reference/) for details of those flags.
In those cases, any arguments passed here will override the config file, if you're using one.
This is a list of all supported flags:

<script setup>
import { data } from '../data-loading/cli-help.data'
</script>

```sh-vue
{{ data.help }}
```

The flags listed below are only available via the command line interface.

### `-c, --config <filename>`

Use the specified config file. If the argument is used but no filename is specified, Rolldown will look for a default config file. See [Configuration Files](#configuration-files) for more details.

### `-h` / `--help`

Show the help message.

### `-v` / `--version`

Show the installed version number.

### `-w` / `--watch`

Rebuild the bundle when source files change on disk.

::: info `ROLLDOWN_WATCH` env
While in watch mode, the `ROLLDOWN_WATCH` and `ROLLUP_WATCH` environment variable will be set to `true` by Rolldown's command line interface and can be checked by other processes. Plugins should instead check [`this.meta.watchMode`](/reference/Interface.PluginContextMeta#watchmode), which is independent of the command line interface.
:::

### `--environment <values>`

Pass additional settings to the config file via `process.env`.
Values are comma-separated key-value pairs, where a value of `true` can be omitted.

For example:

```shell
rolldown -c --environment INCLUDE_DEPS,BUILD:production
```

This will set `process.env.INCLUDE_DEPS = 'true'` and `process.env.BUILD = 'production'`.

You can invoke this option multiple times.
In that case, subsequently set variables will overwrite previous definitions.

::: tip Overwriting the values
If you have `package.json` scripts:

```json
{
  "scripts": {
    "build": "rolldown -c --environment BUILD:production"
  }
}
```

you can call this script with `npm run build -- --environment BUILD:development` to set `process.env.BUILD="development"`.

:::
