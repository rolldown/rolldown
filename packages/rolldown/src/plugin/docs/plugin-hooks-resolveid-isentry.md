::: details Define custom proxy modules for entry points

This can be used for instance as a mechanism to define custom proxy modules for entry points. The following plugin will proxy all entry points to inject a polyfill import.

```js
import { exactRegex } from '@rolldown/pluginutils';
// We prefix the polyfill id with \0 to tell other plugins not to try to load or
// transform it
const POLYFILL_ID = '\0polyfill';
const PROXY_SUFFIX = '?inject-polyfill-proxy';

function injectPolyfillPlugin() {
  return {
    name: 'inject-polyfill',
    async resolveId(source, importer, options) {
      if (source === POLYFILL_ID) {
        // It is important that side effects are always respected for polyfills,
        // otherwise using `treeshake.moduleSideEffects: false` may prevent the
        // polyfill from being included.
        return { id: POLYFILL_ID, moduleSideEffects: true };
      }
      if (options.isEntry) {
        // Determine what the actual entry would have been.
        const resolution = await this.resolve(source, importer, options);
        // If it cannot be resolved or is external, just return it so that Rolldown
        // can display an error
        if (!resolution || resolution.external) return resolution;
        // In the load hook of the proxy, we need to know if the entry has a
        // default export. There, however, we no longer have the full "resolution"
        // object that may contain meta-data from other plugins that is only added
        // on first load. Therefore we trigger loading here.
        const moduleInfo = await this.load(resolution);
        // We need to make sure side effects in the original entry point are
        // respected even for `treeshake.moduleSideEffects: false`. "moduleSideEffects"
        // is a writable property on ModuleInfo.
        moduleInfo.moduleSideEffects = true;
        // It is important that the new entry does not start with `\0` and has the same
        // directory as the original one to not mess up relative external import generation.
        // Also keeping the name and just adding a "?query" to the end ensures that
        // `preserveModules` will generate the original entry name for this entry.
        return `${resolution.id}${PROXY_SUFFIX}`;
      }
      return null;
    },
    load: {
      filter: { id: [exactRegex(POLYFILL_ID), /\?proxy$/] },
      handler(id) {
        if (id === POLYFILL_ID) {
          // Replace with actual polyfill
          return "console.log('polyfill');";
        }
        if (id.endsWith(PROXY_SUFFIX)) {
          const entryId = id.slice(0, -PROXY_SUFFIX.length);
          // We know ModuleInfo.exports is reliable because we awaited this.load in resolveId
          const { exports } = this.getModuleInfo(entryId);
          let code =
            `import ${JSON.stringify(POLYFILL_ID)};` + `export * from ${JSON.stringify(entryId)};`;
          // Namespace reexports do not reexport default, so we need special handling here
          if (exports.includes('default')) {
            code += `export { default } from ${JSON.stringify(entryId)};`;
          }
          return code;
        }
        return null;
      },
    },
  };
}
```

:::
