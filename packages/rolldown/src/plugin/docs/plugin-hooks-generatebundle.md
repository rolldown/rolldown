You can prevent files from being emitted by deleting them from the bundle object in this hook. To emit additional files, use the [`this.emitFile`](/reference/Interface.PluginContext#emitfile) function.

::: danger

Do not directly add assets to the bundle. This will not work as expected as Rolldown will ignore those assets. This is [not recommended in Rollup](https://rollupjs.org/plugin-development/#generatebundle) as well.

Instead, always use [`this.emitFile`](/reference/Interface.PluginContext#emitfile).

:::
