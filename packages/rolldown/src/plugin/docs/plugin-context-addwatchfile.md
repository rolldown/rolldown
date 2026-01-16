Note that when emitting assets that correspond to an existing file, it is recommended to set the [`originalFileName`](/reference/Interface.EmittedAsset#originalfilename) property in the [`this.emitFile`](/reference/Interface.PluginContext#emitfile) call instead as that will not only watch the file but also make the connection transparent to other plugins.

Note: Usually in watch mode to improve rebuild speed, the transform hook will only be triggered for a given module if its contents actually changed. Using `this.addWatchFile` from within the transform hook will make sure the transform hook is also reevaluated for this module if the watched file changes.

In general, it is recommended to use `this.addWatchFile` from within the hook that depends on the watched file.
