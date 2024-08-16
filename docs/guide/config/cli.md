# Command Line Interface

Rolldown supports Command Line Interface (CLI) to rapidly package the application. You can either specify the configuration or directly use the options from the CLI.

## Configuration File

The configuration file can be specified using the `-c` or `--config` option. The configuration file must be a JavaScript file that exports the configuration object.

Due to the API limitations, it is advisable to **pass the option at the last if you intend to disregard the configuration file name**.

```sh
rolldown -c
# OR
rolldown -c rolldown.config.mjs
```

For the time being, the `rolldown.config.js` file can be disregarded.

## Common CLI Options

For the sake of simplicity, we have manually presented the most popular options here. Future enhancements will automatically generate additional options.

### `--dir` / `-d`

Specify the output directory.

```sh
rolldown main.ts -d dist
```

Rolldown will automatically create the directory if it does not already exist.

### `--external` / `-e`

Excluded Module IDs.

For instance, if you wish to exclude the `electron` module from `main.ts`, you can utilize the following command:

```sh
rolldown main.ts -e electron
```

This command will exclude the `electron` module from the bundled output. Please note that we currently only support `string` values for module IDs in the CLI.

### `—format` / `-f`

The output format of the bundled file accepts the following:

- `esm`: ECMAScript Module, including `import`, `export`, and other keywords.
- `cjs`: CommonJS Module, including `require`, `module.exports`, and other keywords.
- `iife`: Immediately Invoked Function Expression, including `window`, `global`, and other keywords.

Future formats will be supported.

**`—minify` or `-m`**

Minify the output file.

```sh
rolldown main.ts -m
```

Rolldown will utilize the `oxc_minify` tool from the [oxc](https://oxc.rs/docs/contribute/minifier.html) to minify the output file. This process is highly efficient.

### `—sourcemap` / `-s`

Generate the source map file.

- If you intend to inline the sourcemap, please use:
  ```shell
  rolldown main.ts -s inline
  ```
- If you wish to emit the sourcemap file, please use:
  ```shell
  rolldown main.ts -s
  ```
  Remember to pass the argument at the end of the command, as per the API limit. We'll try to fix this in the future.
