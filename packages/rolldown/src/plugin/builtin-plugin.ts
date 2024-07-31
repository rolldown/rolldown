import {
  BindingBuiltinPluginName,
  BindingGlobImportPluginConfig,
  BindingBuiltinPlugin,
} from '../binding'

export class BuiltinPlugin {
  constructor(
    public name: BindingBuiltinPluginName,
    public options?: unknown,
  ) {
    this.name = name
    this.options = options
  }
}
export class GlobImportPlugin extends BuiltinPlugin {
  constructor(config?: BindingGlobImportPluginConfig) {
    super(BindingBuiltinPluginName.GlobImportPlugin, config)
  }
}

export class WasmPlugin extends BuiltinPlugin {
  constructor() {
    super(BindingBuiltinPluginName.WasmPlugin)
  }
}

export function globImportPlugin(config?: BindingGlobImportPluginConfig) {
  return new GlobImportPlugin(config)
}

export function wasmPlugin() {
  return new WasmPlugin()
}

export function bindingifyBuiltInPlugin(
  plugin: BuiltinPlugin,
): BindingBuiltinPlugin {
  return {
    __name: plugin.name,
    options: plugin.options,
  }
}
