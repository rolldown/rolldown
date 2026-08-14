If you need to add additional information, you can use the [`meta`](/reference/Interface.RolldownLog#meta) property. If the log contains a [`code`](/reference/Interface.RolldownLog#code) and does not yet have a [`pluginCode`](/reference/Interface.RolldownLog#plugincode) property, it will be renamed to [`pluginCode`](/reference/Interface.RolldownLog#plugincode) as plugin warnings always get a code of `PLUGIN_WARNING` added by Rolldown.

If the logLevel option is set to `"silent"`, this method will do nothing.

::: tip Lazily Compute

If you need to do expensive computations to generate the log, make sure to use the function form so that these computations are only performed if the log is actually processed.

:::
