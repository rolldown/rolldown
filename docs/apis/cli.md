# Command Line Interface

:::warning 🚧 Under Construction
For now this is just the output of `rolldown --help`.
:::

```sh
USAGE rolldown -c <config> or rolldown <input> <options>

OPTIONS

  --config -c, <filename>     Path to the config file (default: rolldown.config.js).
  --dir -d, <dir>             Output directory, defaults to dist if file is not set.
  --external -e, <external>   Comma-separated list of module ids to exclude from the bundle <module-id>,....
  --format -f, <format>       Output format of the generated bundle (supports esm, cjs, and iife).
  --globals -g, <globals>     Global variable of UMD / IIFE dependencies (syntax: key=value).
  --help -h,                  Show help.
  --minify -m,                Minify the bundled file.
  --name -n, <name>           Name for UMD / IIFE format outputs.
  --file -o, <file>           Single output file.
  --platform -p, <platform>   Platform for which the code should be generated (node, browser, neutral).
  --sourcemap -s, <sourcemap> Generate sourcemap (-s inline for inline, or pass the -s on the last argument if you want to generate .map file).
  --version -v,               Show version number.
  --watch -w,                 Watch files in bundle and rebuild on changes.
  --advanced-chunks.min-share-count <advanced-chunks.min-share-count>Minimum share count of the chunk.
  --advanced-chunks.min-size <advanced-chunks.min-size>Minimum size of the chunk.
  --asset-file-names <name>   Name pattern for asset files.
  --banner <banner>           Code to insert the top of the bundled file (outside the wrapper function).
  --checks.circular-dependency Whether to emit warnings when detecting circular dependencies.
  --chunk-file-names <name>   Name pattern for emitted secondary chunks.
  --comments <comments>       Control comments in the output.
  --css-chunk-file-names <css-chunk-file-names>Name pattern for emitted css secondary chunks.
  --css-entry-file-names <css-entry-file-names>Name pattern for emitted css entry chunks.
  --cwd <cwd>                 Current working directory.
  --define <define>           Define global variables.
  --drop-labels <drop-labels> Remove labeled statements with these label names.
  --entry-file-names <name>   Name pattern for emitted entry chunks.
  --es-module                 Always generate __esModule marks in non-ESM formats, defaults to if-default-prop (use --no-esModule to always disable).
  --exports <exports>         Specify a export mode (auto, named, default, none).
  --extend                    Extend global variable defined by name in IIFE / UMD formats.
  --footer <footer>           Code to insert the bottom of the bundled file (outside the wrapper function).
  --hash-characters <hash-characters>Use the specified character set for file hashes.
  --inject <inject>           Inject import statements on demand.
  --inline-dynamic-imports    Inline dynamic imports.
  --intro <intro>             Code to insert the top of the bundled file (inside the wrapper function).
  --jsx.development           Development specific information.
  --jsx.factory <jsx.factory> Jsx element transformation.
  --jsx.fragment <jsx.fragment>Jsx fragment transformation.
  --jsx.import-source <jsx.import-source>Import the factory of element and fragment if mode is classic.
  --jsx.jsx-import-source <jsx.jsx-import-source>Import the factory of element and fragment if mode is automatic.
  --jsx.mode <jsx.mode>       Jsx transformation mode.
  --jsx.refresh               React refresh transformation.
  --log-level <log-level>     Log level (silent, info, debug, warn).
  --module-types <types>      Module types for customized extensions.
  --no-external-live-bindings Disable external live bindings.
  --no-treeshake              Disable treeshaking.
  --outro <outro>             Code to insert the bottom of the bundled file (inside the wrapper function).
  --shim-missing-exports      Create shim variables for missing exports.

EXAMPLES

  1. Bundle with a config file rolldown.config.mjs:
    rolldown -c rolldown.config.mjs

  2. Bundle the src/main.ts to dist with cjs format:
    rolldown src/main.ts -d dist -f cjs

  3. Bundle the src/main.ts and handle the .png assets to Data URL:
    rolldown src/main.ts -d dist --moduleTypes .png=dataurl

  4. Bundle the src/main.tsx and minify the output with sourcemap:
    rolldown src/main.tsx -d dist -m -s

  5. Create self-executing IIFE using external jQuery as $ and _:
    rolldown src/main.ts -d dist -n bundle -f iife -e jQuery,window._ -g jQuery=$

NOTES

  * Due to the API limitation, you need to pass -s for .map sourcemap file as the last argument.
  * If you are using the configuration, please pass the -c as the last argument if you ignore the default configuration file.
  * CLI options will override the configuration file.
  * For more information, please visit https://rolldown.rs/.
```
