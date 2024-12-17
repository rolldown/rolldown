import { ModuleOptions } from '..'

export interface ModuleInfo extends ModuleOptions {
  /**
   *  Unsupported at rolldown
   */
  ast: any
  code: string | null
  id: string
  importers: string[]
  dynamicImporters: string[]
  importedIds: string[]
  dynamicallyImportedIds: string[]
  exports: string[]
  isEntry: boolean
}
