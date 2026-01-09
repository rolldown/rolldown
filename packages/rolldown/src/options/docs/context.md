#### In-depth

The `context` option controls what `this` refers to in the top-level scope of the input modules.

In ES modules, the `this` value is `undefined` by specification. This option allows you to set a different value. For example, if your input modules expect `this` to be `window` like in non-ES module scripts, you can set `context` to `'window'`.

Note that if the input module is detected as CommonJS, Rolldown will use `exports` as the `this` value regardless of this option.
