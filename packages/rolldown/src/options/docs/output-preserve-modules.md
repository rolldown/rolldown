Instead of creating as few chunks as possible, this mode will create separate chunks for all modules using the original module names as file names. Tree-shaking will still be applied, suppressing files that are not used by the provided entry points or do not have side effects when executed and removing unused exports of files that are not entry points. On the other hand, if plugins emit additional "virtual" files to achieve certain results, those files will be emitted as actual files using a pattern [`${output.virtualDirname}/fileName.js`](/reference/OutputOptions.virtualDirname).

It is therefore not recommended to blindly use this option to transform an entire file structure to another format if you directly want to import from those files as expected exports may be missing. In that case, you should rather designate all files explicitly as entry points by adding them to the [`input`](/reference/InputOptions.input) option object.

::: details Example of designating all files as entry points

You can do so dynamically, e.g., via the [`tinyglobby`](https://github.com/SuperchupuDev/tinyglobby) package:

```js
import { defineConfig } from 'rolldown';
import { globSync } from 'tinyglobby';
import path from 'node:path';

export default defineConfig({
  input: Object.fromEntries(
    globSync('src/**/*.js').map((file) => [
      // This removes `src/` as well as the file extension from each
      // file, so e.g., src/nested/foo.js becomes nested/foo
      path.relative('src', file.slice(0, file.length - path.extname(file).length)),
      // This expands the relative paths to absolute paths, so e.g.,
      // src/nested/foo becomes /project/src/nested/foo.js
      path.resolve(file),
    ]),
  ),
  output: {
    format: 'es',
    dir: 'dist',
  },
});
```

:::
