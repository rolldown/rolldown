::: warning Changing `moduleType`

When you change the [type of the module](/in-depth/module-types) by returning [`moduleType`](/reference/Interface.SourceDescription#moduletype) property, the module is not thrown back to the beginning of the plugin chain. This means the `transform` hooks of the plugins that already saw this module will not be called with the new `moduleType`. For this reason, it is recommended to place the plugins that change the `moduleType` at the beginning of the plugin list.

If you need to let all the plugins be called, you can create a [virtual module](/apis/plugin-api#virtual-modules) with a different `moduleType` instead of changing the `moduleType` directly in the `transform` hook.

:::
