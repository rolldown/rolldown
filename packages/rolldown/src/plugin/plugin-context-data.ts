import { BindingPluginContext } from '../binding'
import { ModuleOptions } from '..'
import { transformModuleInfo } from '../utils/transform-module-info'
import { PluginContextResolveOptions } from './plugin-context'
import type { ModuleInfo } from '../types/module-info'

export class PluginContextData {
  moduleOptionMap: Map<string, ModuleOptions> = new Map()
  resolveOptionsMap: Map<number, PluginContextResolveOptions> = new Map()
  loadModulePromiseMap: Map<string, Promise<void>> = new Map()

  updateModuleOption(id: string, option: ModuleOptions): void {
    const existing = this.moduleOptionMap.get(id)
    if (existing) {
      if (option.moduleSideEffects != null) {
        existing.moduleSideEffects = option.moduleSideEffects
      }
      if (option.meta != null) {
        Object.assign(existing.meta, option.meta)
      }
    } else {
      this.moduleOptionMap.set(id, option)
    }
  }

  getModuleOption(id: string): ModuleOptions {
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

  getModuleInfo(id: string, context: BindingPluginContext): ModuleInfo | null {
    const bindingInfo = context.getModuleInfo(id)
    if (bindingInfo) {
      const info = transformModuleInfo(bindingInfo, this.getModuleOption(id))
      return info
    }
    return null
  }

  getModuleIds(context: BindingPluginContext): ArrayIterator<string> {
    const moduleIds = context.getModuleIds()
    return moduleIds.values()
  }

  saveResolveOptions(options: PluginContextResolveOptions): number {
    const index = this.resolveOptionsMap.size
    this.resolveOptionsMap.set(index, options)
    return index
  }

  getSavedResolveOptions(
    receipt: number,
  ): PluginContextResolveOptions | undefined {
    return this.resolveOptionsMap.get(receipt)
  }

  removeSavedResolveOptions(receipt: number): void {
    this.resolveOptionsMap.delete(receipt)
  }
}
