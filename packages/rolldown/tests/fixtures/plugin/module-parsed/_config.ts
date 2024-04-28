import { defineTest } from '@tests'
import { expect } from 'vitest'
import path from 'node:path'

const moduleInfos: any[] = []

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin',
        moduleParsed: function (moduleInfo) {
          expect(moduleInfo.code).not.toBeUndefined()
          expect(moduleInfo.id).not.toBeUndefined()
          if (moduleInfo.id.endsWith('main.js')) {
            expect(moduleInfo.isEntry).toBe(true)
            expect(moduleInfo.importedIds).toStrictEqual([
              path.join(import.meta.dirname, 'static.js'),
            ])
            expect(moduleInfo.dynamicallyImportedIds).toStrictEqual([
              path.join(import.meta.dirname, 'dynamic.js'),
            ])
            expect(moduleInfo.importers).toStrictEqual([])
            expect(moduleInfo.dynamicImporters).toStrictEqual([])
          } else {
            expect(moduleInfo.isEntry).toBe(false)
            expect(moduleInfo.importedIds).toStrictEqual([])
            expect(moduleInfo.dynamicallyImportedIds).toStrictEqual([])
            expect(moduleInfo.importers).toStrictEqual([])
            expect(moduleInfo.dynamicImporters).toStrictEqual([])
          }
          moduleInfos.push(moduleInfo)
        },
        buildEnd() {
          // TODO The `importers` and `dynamicallyImportedIds` should has valid values at rollup, we need to find a way to resolve it.
          //  for (const moduleInfo of moduleInfos) {
          //   switch (moduleInfo.id) {
          //     case path.join(import.meta.dirname, 'main.js'):
          //       expect(moduleInfo.importedIds).toStrictEqual([path.join(import.meta.dirname, 'static.js')])
          //       expect(moduleInfo.dynamicallyImportedIds).toStrictEqual([path.join(import.meta.dirname, 'dynamic.js')])
          //       expect(moduleInfo.importers).toStrictEqual([])
          //       expect(moduleInfo.dynamicImporters).toStrictEqual([])
          //       break;
          //     case path.join(import.meta.dirname, 'static.js'):
          //       expect(moduleInfo.importedIds).toStrictEqual([])
          //       expect(moduleInfo.dynamicallyImportedIds).toStrictEqual([])
          //       expect(moduleInfo.importers).toStrictEqual([path.join(import.meta.dirname, 'main.js')])
          //       expect(moduleInfo.dynamicImporters).toStrictEqual([])
          //       break;
          //     case path.join(import.meta.dirname, 'dynamic.js'):
          //       expect(moduleInfo.importedIds).toStrictEqual([])
          //       expect(moduleInfo.dynamicallyImportedIds).toStrictEqual([])
          //       expect(moduleInfo.importers).toStrictEqual([])
          //       expect(moduleInfo.dynamicImporters).toStrictEqual([path.join(import.meta.dirname, 'main.js')])
          //       break;
          //   }
          //  }
        },
      },
    ],
  },
})
