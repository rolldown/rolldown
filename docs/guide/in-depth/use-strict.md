# `"use strict"`

Before we dive into the details, let's first add a bit of context.

- ESM (ECMAScript Modules) is always in strict mode implicitly. [link](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Strict_mode#strict_mode_for_modules).

This means that if you're using ESM, you don't need to add `"use strict"` at the top of your files. It's enabled by default.

- Strict mode isn't just a subset: it intentionally has different semantics from normal code.

## `format: 'esm'`

You don't need to care about `"use strict"`, if all your files are ESM.

Things will be a bit complicated, if your files contain commonjs modules. Since you want to emit ESM output, you have to ensure that your code of commonjs files is valid in strict mode. Rolldown will parse it in strict mode, and will throw an error if it's not valid.

With `format: 'esm'`, rolldown will parse you code in strict mode, and will throw an error that is related to strict mode. After parsing, rolldown will simply removes `"use strict"` from every module if it's present.

Emitted code will not contain `"use strict"`, because ESM is already in strict mode.

## `format: 'cjs'`

There will be several strategies to handle `"use strict"` while emitting commonjs output.

Rollup will always add `"use strict"` at the top of the output file, because it only accepts ESM input.

Esbuild emit `"use strict"` conditionally([link](https://github.com/evanw/esbuild/issues/2264#issuecomment-1138927861)) but not perfectly.

Esbuild also explains that it's almost not possible to handle `"use strict"` perfectly with mixed ESM and CJS modules with enabling scope hoisting([link](https://github.com/evanw/esbuild/issues/2381#issuecomment-1179765091)).

Rolldown choose to emit runnable code in maximum possibility.

Rolldown will only add `"use strict"` in each chunk, if all modules of that chunk are satisfied in following conditions:

- ESM
- Commonjs module with `"use strict"` at the top of the file

Otherwise, it will not add `"use strict"` in the chunk. Notice that, this changes the semantics of the original ESM code, because `"use strict"` is not added in the chunk.

With `format: 'cjs'`, rolldown will first parse your code in strict mode, and will re-parse it in non-strict mode if it throws an error in the first parsing.

See here for more details about what are affected by `"use strict"` [link](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Strict_mode#changes_in_strict_mode).
