#### In-depth

Rolldown uses Oxc under the hood for transformation.

While Oxc does not support lowering the latest decorators proposal yet, Rolldown is able to bundle them.

#### Browserslist

`transform.target` accepts [Browserslist queries](https://github.com/browserslist/browserslist#full-list), resolved by [oxc-browserslist](https://github.com/oxc-project/oxc-browserslist).

For example, set it to `baseline widely available` to lower JavaScript for browsers that support the [Baseline Widely Available](https://web-platform-dx.github.io/web-features/) feature set:

```js [rolldown.config.js]
export default defineConfig({
  transform: {
    target: 'baseline widely available',
  },
});
```

The selected browser versions can change when Rolldown updates its bundled Browserslist data. For a reproducible target, use a dated query such as `baseline widely available on 2026-08-26`.

The target controls JavaScript syntax lowering. It does not add polyfills for web APIs.
