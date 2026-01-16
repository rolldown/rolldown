This allows you to inspect the final content of modules before deciding how to resolve them in the [`resolveId`](/reference/Interface.Plugin#resolveid) hook and e.g. resolve to a proxy module instead. If the module becomes part of the graph later, there is no additional overhead from using this context function as the module will not be parsed again. The signature allows you to directly pass the return value of [`this.resolve`](/reference/Interface.PluginContext#resolve) to this function as long as it is neither `null` nor external.

The returned Promise will resolve once the module has been fully transformed and parsed but before any imports have been resolved. That means that the resulting [`ModuleInfo`](/reference/Interface.ModuleInfo) will have empty [`importedIds`](/reference/Interface.ModuleInfo#importedids) and [`dynamicallyImportedIds`](/reference/Interface.ModuleInfo#dynamicallyimportedids). This helps to avoid deadlock situations when awaiting `this.load` in a [`resolveId`](/reference/Interface.Plugin#resolveid) hook. If you are interested in [`importedIds`](/reference/Interface.ModuleInfo#importedids) and [`dynamicallyImportedIds`](/reference/Interface.ModuleInfo#dynamicallyimportedids), you can either implement a [`moduleParsed`](/reference/Interface.Plugin#moduleparsed) hook or pass the `resolveDependencies` flag, which will make the Promise returned by `this.load` wait until all dependency ids have been resolved.

Note that with regard to the `meta` and `moduleSideEffects` options, the same restrictions apply as for the [`resolveId`](/reference/Interface.Plugin#resolveid) hook: Their values only have an effect if the module has not been loaded yet. Thus, it is very important to use [`this.resolve`](/reference/Interface.PluginContext#resolve) first to find out if any plugins want to set special values for these options in their [`resolveId`](/reference/Interface.Plugin#resolveid) hook, and pass these options on to `this.load` if appropriate. The example below showcases how this can be handled to add a proxy module for modules containing a special code comment. Note the special handling for re-exporting the default export:

```js
export default function addProxyPlugin() {
  return {
    async resolveId(source, importer, options) {
      if (importer?.endsWith('?proxy')) {
        // Do not proxy ids used in proxies
        return null;
      }
      // We make sure to pass on any resolveId options to
      // this.resolve to get the module id
      const resolution = await this.resolve(source, importer, options);
      // We can only pre-load existing and non-external ids
      if (resolution && !resolution.external) {
        // we pass on the entire resolution information
        const moduleInfo = await this.load(resolution);
        if (moduleInfo.code.includes('/* use proxy */')) {
          return `${resolution.id}?proxy`;
        }
      }
      // As we already fully resolved the module, there is no reason
      // to resolve it again
      return resolution;
    },
    load: {
      filter: { id: /\?proxy$/ },
      handler(id) {
        const importee = id.slice(0, -'?proxy'.length);
        // Note that namespace reexports do not reexport default exports
        let code =
          `console.log('proxy for ${importee}'); ` + `export * from ${JSON.stringify(importee)};`;
        // We know that while resolving the proxy, importee was
        // already fully loaded and parsed, so we can rely on `exports`
        if (this.getModuleInfo(importee).exports.includes('default')) {
          code += `export { default } from ${JSON.stringify(importee)};`;
        }
        return code;
      },
    },
  };
}
```

If the module was already loaded, `this.load` will just wait for the parsing to complete and then return its module information. If the module was not yet imported by another module, it will not automatically trigger loading other modules imported by this module. Instead, static and dynamic dependencies will only be loaded once this module has actually been imported at least once.

::: warning Deadlocks caused by awaiting `this.load` in cyclic dependencies

While it is safe to use `this.load` in a [`resolveId`](/reference/Interface.Plugin#resolveid) hook, you should be very careful when awaiting it in a [`load`](/reference/Interface.Plugin#load) or [`transform`](/reference/Interface.Plugin#transform) hook. If there are cyclic dependencies in the module graph, this can easily lead to a deadlock, so any plugin needs to manually take care to avoid waiting for `this.load` inside the [`load`](/reference/Interface.Plugin#load) or [`transform`](/reference/Interface.Plugin#transform) of the any module that is in a cycle with the loaded module.

:::
