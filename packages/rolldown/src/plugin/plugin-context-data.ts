import { BindingPluginContext } from '../binding'
import { CustomPluginOptions, ModuleInfo, ModuleOptions } from '..'
import { transformModuleInfo } from '../utils/transform-module-info'

export class PluginContextData {
  modules = new Map<string, ModuleInfo>()
  moduleIds: Array<string> | null = null
  moduleOptionMap = new Map<string, ModuleOptions>()
  resolveCustomMap = new Map<number, CustomPluginOptions>()

  updateModuleOption(id: string, option: ModuleOptions) {
    const existing = this.moduleOptionMap.get(id)
    if (existing) {
      Object.assign(existing, option)
      if (option.meta != null) {
        Object.assign(existing.meta, option.meta)
      }
    } else {
      this.moduleOptionMap.set(id, option)
    }
  }

  getModuleOption(id: string) {
    return this.moduleOptionMap.get(id)
  }

  getModuleInfo(id: string, context: BindingPluginContext) {
    if (this.modules.has(id)) {
      return this.modules.get(id) ?? null
    }
    const bindingInfo = context.getModuleInfo(id)
    if (bindingInfo) {
      const info = transformModuleInfo(
        bindingInfo,
        this.moduleOptionMap.get(id)!,
      )
      this.modules.set(id, info)
      return info
    }
    return null
  }

  getModuleIds(context: BindingPluginContext) {
    if (this.moduleIds) {
      return this.moduleIds.values()
    }
    const moduleIds = context.getModuleIds()
    if (moduleIds) {
      this.moduleIds = moduleIds
      return moduleIds.values()
    }
    return [].values()
  }

  setResolveCustom(custom: CustomPluginOptions): number {
    const index = Object.keys(this.resolveCustomMap).length
    this.resolveCustomMap.set(index, custom)
    return index
  }

  getResolveCustom(index: number) {
    return this.resolveCustomMap.get(index)
  }
}
