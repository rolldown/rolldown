import {
  BindingBuiltinGlobImportPlugin,
  BindingGlobImportPluginConfig,
  BindingBuiltinWasmPlugin,
} from '../binding'

interface ToBindingBuiltinPlugin {
  toBuiltIn: () => any
}
export class BuiltinPlugin {
  constructor(public config?: any) {
    this.config = config
  }
  toBuiltIn(): any {
    throw new Error('Method not implemented.')
  }
}

export class BuiltinGlobImportPlugin
  extends BuiltinPlugin
  implements ToBindingBuiltinPlugin
{
  constructor(config?: BindingGlobImportPluginConfig) {
    super(config)
  }
  toBuiltIn() {
    return {
      config: this.config,
    }
  }
}

export class BuiltinWasmPlugin
  extends BuiltinPlugin
  implements ToBindingBuiltinPlugin
{
  constructor() {
    super()
  }
  toBuiltIn(): BindingBuiltinWasmPlugin {
    return {}
  }
}

/**
 * @param plugin
 * @returns could be any built plugin
 * */
export function bindingifyBuiltInPlugin(
  plugin: BuiltinPlugin,
): BindingBuiltinGlobImportPlugin | BindingBuiltinWasmPlugin {
  return plugin.toBuiltIn()
}
