import {
  BindingBuiltinGlobImportPlugin,
  BindingGlobImportPluginConfig,
} from '../binding'

interface ToBindingBuiltinPlugin {
  toBuiltIn: () => any
}
export class BuiltinPlugin implements ToBindingBuiltinPlugin {
  constructor(public config?: any) {
    this.config = config
  }
  toBuiltIn() {
    return {
      config: this.config,
    }
  }
}

export class BuiltinGlobImportPlugin extends BuiltinPlugin {
  constructor(config: BindingGlobImportPluginConfig) {
    super(config)
  }
}

/**
 * @param plugin
 * @returns could be any built plugin
 * */
export function bindingifyBuiltInPlugin(
  plugin: BuiltinPlugin,
): BindingBuiltinGlobImportPlugin {
  return plugin.toBuiltIn()
}
