# Troubleshooting

## Performance

Performance is a primary goal for Rolldown. However, build performance isn't solely determined by Rolldown itself. It's also significantly affected by the environment it runs in and the plugins used.

While we continuously strive to improve Rolldown to minimize these external factors, there are inherent limitations and areas where optimizations are still ongoing. This guide provides insights into potential bottlenecks and how you can mitigate them.

### Environment

The operating system and its configuration can impact build times, particularly file system operations.

#### Windows

File system access on Windows is generally slower compared to other operating systems like macOS or Linux. Especially, antivirus software can make this much worse. But even without interference from antivirus programs, baseline file system performance tends to be slower. It is 3 times slower than macOS and 10 times slower than Linux. This becomes a bottleneck when most of the transforms are done without a plugin.

To improve performance on Windows, consider using alternative file system environments:

1. [**Dev Drive**](https://learn.microsoft.com/en-us/windows/dev-drive/): A newer Windows feature designed for developer workloads, using the Resilient File System (ReFS). Using a Dev Drive can lead to a **2x to 3x speedup** compared to the standard Windows NTFS file system for file system operations.
2. [**Windows Subsystem for Linux (WSL)**](https://learn.microsoft.com/en-us/windows/wsl/): WSL lets Linux environment to run on Windows easily, which offers significantly better file system performance. Placing your project files and running the build process within WSL can result in speedups of around **10x** compared to the standard Windows NTFS file system for file system operations.

:::details Benchmark Reference

The benchmark script used is described in this blog post ([How fast can you open 1000 files?](https://lemire.me/blog/2025/03/01/how-fast-can-you-open-1000-files/)).

The results were:

|    File System / Threads |     1 |     2 |     4 |     8 |    16 |
| -----------------------: | ----: | ----: | ----: | ----: | ----: |
|             Windows NTFS | 286ms | 153ms |  85ms | 106ms | 110ms |
| Windows Dev Drive (ReFS) | 124ms |  67ms |  35ms |  48ms |  55ms |
|               WSL (ext4) |  24ms |  13ms | 7.8ms | 9.0ms |  13ms |

The benchmark was ran on the following environment:

- OS: Windows 11 Pro 23H2 22631.5189
- CPU: AMD Ryzen 9 5900X
- Memory: DDR4-3600 32GB
- SSD: Western Digital Black SN850X 1TB

:::

<!-- Maybe write about macOS as well? -->

### Plugins

Plugins extend Rolldown's functionality, but can also introduce performance overhead.

#### Plugin Hook Filters

Rolldown provides a feature called **Plugin Hook Filters**. This allows you to specify precisely which modules a plugin hook should process, reducing the communication overhead between JavaScript and Rust. For detailed information on how filters work internally, refer to the [Plugin Development Guide - Hook Filters](/guide/plugin-development#plugin-hook-filters).

If you are a plugin user and the plugin you use does not have hook filters specified, you can apply them by using the `withFilter` utility function exported by Rolldown.

```js
import yaml from '@rollup/plugin-yaml';
import { defineConfig, withFilter } from 'rolldown';

export default defineConfig({
  plugins: [
    // Run the transform hook of the `yaml` plugin only for modules which end in `.yaml`
    withFilter(
      yaml({
        /*...*/
      }),
      { transform: { id: /\.yaml$/ } },
    ),
  ],
});
```

#### Leverage Built-in Features

Rolldown includes several built-in features designed for efficiency. Where possible, prefer using these native capabilities over external Rollup plugins that perform similar tasks. Relying on built-in functionality often means the processing happens entirely within Rust, allowing to process in parallel.

Check the [Rolldown Features](/guide/features) page for capabilities that does not exist in Rollup.

For example, the following common Rollup plugins may be replaced with Rolldown's built-in features:

- `@rollup/plugin-alias`: [`resolve.alias`](/reference/config-options#resolve-alias) option
- `@rollup/plugin-commonjs`: supported out of the box
- `@rollup/plugin-inject`: [`inject`](/reference/config-options#inject) option
- `@rollup/plugin-node-resolve`: supported out of the box
- `@rollup/plugin-json`: supported out of the box
- `@rollup/plugin-swc`, `@rollup/plugin-babel`, `@rollup/plugin-sucrase`: supported out of the box via Oxc (complex configurations might still require the plugin)
- `@rollup/plugin-terser`: `output.minify` option

<!--
experimental plugins (do we want to document these?)

- `@rollup/plugin-replace`: `import { replacePlugin } from 'rolldown/experimental'`
- `@rollup/plugin-dynamic-import-vars`: `import { dynamicImportVarsPlugin } from 'rolldown/experimental'`

-->
