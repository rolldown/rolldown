// import 'types/importMeta'

export interface ModuleType {
  name: string
}

export const basic = import.meta.glob<ModuleType>('./modules/*.ts')
export const basicWithObjectKeys = Object.keys(import.meta.glob<ModuleType>('./modules/*.ts'))
export const basicWithObjectValues = Object.values(import.meta.glob<ModuleType>('./modules/*.ts'))

export const basicEager = import.meta.glob<ModuleType>('./modules/*.ts', {
  eager: true,
})
export const basicEagerWithObjectKeys = Object.keys(
  import.meta.glob<ModuleType>('./modules/*.ts', {
    eager: true,
  }),
)
export const basicEagerWithObjectValues = Object.values(
  import.meta.glob<ModuleType>('./modules/*.ts', {
    eager: true,
  }),
)

export const ignore = import.meta.glob(['./modules/*.ts', '!**/index.ts'])
export const ignoreWithObjectKeys = Object.keys(
  import.meta.glob(['./modules/*.ts', '!**/index.ts']),
)
export const ignoreWithObjectValues = Object.values(
  import.meta.glob(['./modules/*.ts', '!**/index.ts']),
)

export const namedEager = import.meta.glob<string>('./modules/*.ts', {
  eager: true,
  import: 'name',
})
export const namedEagerWithObjectKeys = Object.keys(
  import.meta.glob<string>('./modules/*.ts', {
    eager: true,
    import: 'name',
  }),
)
export const namedEagerWithObjectValues = Object.values(
  import.meta.glob<string>('./modules/*.ts', {
    eager: true,
    import: 'name',
  }),
)

export const namedDefault = import.meta.glob<string>('./modules/*.ts', {
  import: 'default',
})
export const namedDefaultWithObjectKeys = Object.keys(
  import.meta.glob<string>('./modules/*.ts', {
    import: 'default',
  }),
)
export const namedDefaultWithObjectValues = Object.values(
  import.meta.glob<string>('./modules/*.ts', {
    import: 'default',
  }),
)

export const eagerAs = import.meta.glob<ModuleType>(
  ['./modules/*.ts', '!**/index.ts'],
  { eager: true, query: '?raw', import: 'default' },
)

export const rawImportModule = import.meta.glob(
  ['./modules/*.ts', '!**/index.ts'],
  { query: '?raw', import: '*' },
)

export const excludeSelf = import.meta.glob(
  './*.ts',
  // for test: annotation contain ")"
  /*
   * for test: annotation contain ")"
   * */
)
export const excludeSelfRaw = import.meta.glob('./*.ts', { query: '?raw' })

export const customQueryString = import.meta.glob('./*.ts', { query: 'base64' })

export const parent = import.meta.glob('../../playground/src/*.ts', {
  query: '?url',
  import: 'default',
})

export const rootMixedRelative = import.meta.glob(['/*.ts', '../b/*.ts'], {
  query: '?url',
  import: 'default',
})

export const cleverCwd1 = import.meta.glob(
  './node_modules/framework/**/*.page.js',
)

export const cleverCwd2 = import.meta.glob([
  './modules/*.ts',
  '../b/*.ts',
  '!**/index.ts',
])

export const customBase = import.meta.glob('./**/*.ts', { base: './' })

export const customRootBase = import.meta.glob('./**/*.ts', {
  base: '/b',
})

export const customBaseParent = import.meta.glob('/b/**/*.ts', {
  base: '/a',
})