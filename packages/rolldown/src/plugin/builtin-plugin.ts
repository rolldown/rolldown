import { BindingBuiltinPlugin, BindingBuiltinPluginName } from '../binding'

export class BuiltinPlugin {
  constructor(
    public name: BindingBuiltinPluginName,
    public options?: unknown,
  ) {
    this.name = name
    this.options = options
  }
}

export class BuiltinWasmPlugin extends BuiltinPlugin {
  constructor(options?: unknown) {
    super(BindingBuiltinPluginName.Wasm, options)
  }
}

export class BuiltinDynamicImportVarsPlugin extends BuiltinPlugin {
  constructor(options?: unknown) {
    super(BindingBuiltinPluginName.DynamicImportVars, options)
  }
}

export class BuiltinGlobImportPlugin extends BuiltinPlugin {
  constructor(options?: unknown) {
    super(BindingBuiltinPluginName.GlobImport, options)
  }
}

export function bindingifyBuiltInPlugin(
  plugin: BuiltinPlugin,
): BindingBuiltinPlugin {
  return {
    name: plugin.name,
    options: plugin.options,
  }
}
