import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'
import path from 'node:path'
import { GetModuleInfo } from 'rolldown'

const meta = { value: 1 }
export default defineTest({
  config: {
    external: ['external'],
    plugins: [
      {
        name: 'test-plugin-context',
        resolveId(id) {
          if (id.endsWith('main.js')) {
            return {
              id: path.join(__dirname, 'main.js'),
              meta,
            }
          }
        },
        load(id) {
          if (id.endsWith('main.js')) {
            const moduleInfo = this.getModuleInfo(id)!
            expect(moduleInfo.isEntry).toBe(true)
          }
          if (id.endsWith('static.js')) {
            const moduleInfo = this.getModuleInfo(id)!
            expect(moduleInfo.meta).toBeInstanceOf(Object)
          }
        },
        renderStart() {
          testGetModuleInfoThis(this.getModuleInfo)
          let count = 0
          for (const id of this.getModuleIds()) {
            count++
            const moduleInfo = this.getModuleInfo(id)!
            switch (moduleInfo.id) {
              case path.join(import.meta.dirname, 'main.js'):
                expect(moduleInfo.importedIds).toStrictEqual([
                  'external',
                  path.join(import.meta.dirname, 'static.js'),
                ])
                expect(moduleInfo.dynamicallyImportedIds).toStrictEqual([
                  path.join(import.meta.dirname, 'dynamic.js'),
                ])
                expect(moduleInfo.importers).toStrictEqual([])
                expect(moduleInfo.dynamicImporters).toStrictEqual([])
                expect(moduleInfo.meta).toBe(meta)
                expect(moduleInfo.exports).toStrictEqual(['result', '*'])
                break
              case path.join(import.meta.dirname, 'static.js'):
                expect(moduleInfo.importedIds).toStrictEqual([])
                expect(moduleInfo.dynamicallyImportedIds).toStrictEqual([])
                expect(moduleInfo.importers).toStrictEqual([
                  path.join(import.meta.dirname, 'main.js'),
                ])
                expect(moduleInfo.dynamicImporters).toStrictEqual([])
                break
              case path.join(import.meta.dirname, 'dynamic.js'):
                expect(moduleInfo.importedIds).toStrictEqual([])
                expect(moduleInfo.dynamicallyImportedIds).toStrictEqual([])
                expect(moduleInfo.importers).toStrictEqual([])
                expect(moduleInfo.dynamicImporters).toStrictEqual([
                  path.join(import.meta.dirname, 'main.js'),
                ])
                break

              case 'external':
                expect(moduleInfo.id).toBe('external')
                break
            }
          }
          expect(count).toBe(4)
        },
      },
    ],
  },
})

function testGetModuleInfoThis(getModuleInfo: GetModuleInfo) {
  const moduleInfo = getModuleInfo(path.join(import.meta.dirname, 'main.js'))!
  expect(moduleInfo.isEntry).toBe(true)
}
