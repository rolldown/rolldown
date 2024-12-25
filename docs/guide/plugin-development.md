# Plugin Development

Rolldown's plugin interface is almost fully compatible with Rollup's (detailed tacking [here](https://github.com/rolldown/rolldown/issues/819)), so if you have written a Rollup plugin before, you already know how to write a Rolldown plugin!

We are still working on creating a more detailed guide for users who are new to both Rollup and Rolldown. For now, please first refer to [Rollup's plugin development guide](https://rollupjs.org/plugin-development/).

## Plugin hook filters

One important thing to note for JavaScript plugins in Rolldown is that every plugin hook call incurs a small communication overhead between Rust and the JavaScript runtime (i.e. Node.js).

Consider the following plugin:

```js{5}
export default function myPlugin() {
  return {
    name: 'example',
    transform(code, id) {
      if (!id.endsWith('.data')) {
        // early return
        return
      }
      // preform actual transform
      return transformedCode
    },
  }
}
```

Line 5 is a very common pattern in Rollup plugins: check the file extension of a module inside the plugin hook to determine whether it needs to be processed.

However, this would be sub-optimal in a Rust bundler like Rolldown. Imagine this plugin is used in a large app with thousands of modules - Rolldown would have to invoke a Rust-to-JS call for every module, and in many cases just for the plugin to find out it doesn't actually need to do anything. Due to the single-threaded nature of JavaScript, unnecessary Rust-to-JS calls can also de-optimize parallelization.

Ideally, we want to be able to determine whether a plugin hook needs to be invoked at all without leaving Rust. This is why Rolldown augments Rollup plugin's object hook format with the additional `filter` property. The above plugin can be updated to:

```js{5}
export default function myPlugin() {
  return {
    name: 'example',
    transform: {
      filter: {
        id: /\.data$/
      },
      handler (code) {
        // preform actual transform
        return transformedCode
      },
    }
  }
}
```

Rolldown can now compile and execute the regular expression on the Rust side, and can avoid invoking JS if the filter does not match.

In addition to `id`, you can also filter based on `moduleType` and the module's source code. Full `HookFilter` interface for the `filter` property:

````ts
interface HookFilter {
  /**
   * This filter is used to do a pre-test to determine whether the hook should be called.
   * @example
   * // Filter out all `id`s that contain `node_modules` in the path.
   * ```js
   * { id: 'node_modules' }
   * ```
   * @example
   * // Filter out all `id`s that contain `node_modules` or `src` in the path.
   * ```js
   * { id: ['node_modules', 'src'] }
   * ```
   * @example
   * // Filter out all `id`s that start with `http`
   * ```js
   * { id: /^http/ }
   * ```
   * @example
   * // Exclude all `id`s that contain `node_modules` in the path.
   * ```js
   * { id: { exclude: 'node_modules' } }
   * ```
   * @example
   * // Formal pattern
   * ```
   * { id : {
   *   include: ["foo", /bar/],
   *   exclude: ["baz", /qux/]
   * }}
   * ```
   */
  id?: StringFilter
  moduleType?: ModuleTypeFilter
  code?: StringFilter
}
````
