import type { RolldownOptions } from '../types/rolldown-options'
import type { ConfigExport } from '../types/config-export'

export function defineConfig(config: RolldownOptions): RolldownOptions
export function defineConfig(config: RolldownOptions[]): RolldownOptions[]
export function defineConfig(config: ConfigExport): ConfigExport
export function defineConfig(config: ConfigExport): ConfigExport {
  return config
}
