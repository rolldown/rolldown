import { BindingPluginContext } from '../binding'
import { ModuleOptions } from '..'
import { transformModuleInfo } from '../utils/transform-module-info'
import { PluginContextResolveOptions } from './plugin-context'

export class PluginContextData {
  moduleOptionMap = new Map<string, ModuleOptions>()
  resolveOptionsMap = new Map<number, PluginContextResolveOptions>()
  loadModulePromiseMap: Map<string, Promise<void>> = new Map()

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
    const option = this.moduleOptionMap.get(id)
    if (!option) {
      const raw: ModuleOptions = {
        moduleSideEffects: null,
        meta: {},
      }
      this.moduleOptionMap.set(id, raw)
      return raw
    }
    return option
  }

  getModuleInfo(id: string, context: BindingPluginContext) {
    const bindingInfo = context.getModuleInfo(id)
    if (bindingInfo) {
      const info = transformModuleInfo(bindingInfo, this.getModuleOption(id))
      return info
    }
    return null
  }

  getModuleIds(context: BindingPluginContext) {
    const moduleIds = context.getModuleIds()
    return moduleIds.values()
  }

  saveResolveOptions(options: PluginContextResolveOptions): number {
    const index = this.resolveOptionsMap.size
    this.resolveOptionsMap.set(index, options)
    return index
  }

  getSavedResolveOptions(receipt: number) {
    return this.resolveOptionsMap.get(receipt)
  }

  removeSavedResolveOptions(receipt: number) {
    this.resolveOptionsMap.delete(receipt)
  }
}
