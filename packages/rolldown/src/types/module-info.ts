export interface ModuleInfo {
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
  isEntry: boolean
}
