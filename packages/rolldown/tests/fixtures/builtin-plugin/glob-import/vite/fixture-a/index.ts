// import 'types/importMeta'

export interface ModuleType {
  name: string
}

export const basic = import.meta.glob<ModuleType>('./modules/*.ts')
// todo: omit values
// vite output: Object.keys({"./modules/a.ts": 0,"./modules/b.ts": 0,"./modules/index.ts": 0});
// prettier-ignore
export const basicWithObjectKeys = Object.keys(import.meta.glob<ModuleType>('./modules/*.ts'))
// todo: omit keys
// vite output: Object.values([() => import("./modules/a.ts"),() => import("./modules/b.ts"),() => import("./modules/index.ts")]
// prettier-ignore
export const basicWithObjectValues = Object.values(import.meta.glob<ModuleType>('./modules/*.ts'))

export const basicEager = import.meta.glob<ModuleType>('./modules/*.ts', {
  eager: true,
})
// todo: omit values
// vite output: Object.keys({"./modules/a.ts": 0,"./modules/b.ts": 0,"./modules/index.ts": 0});
export const basicEagerWithObjectKeys = Object.keys(
  import.meta.glob<ModuleType>('./modules/*.ts', {
    eager: true,
  }),
)
// todo: omit keys
// vite output: Object.values([__vite_glob_5_0,__vite_glob_5_1,__vite_glob_5_2]);
export const basicEagerWithObjectValues = Object.values(
  import.meta.glob<ModuleType>('./modules/*.ts', {
    eager: true,
  }),
)

export const ignore = import.meta.glob(['./modules/*.ts', '!**/index.ts'])
// todo: omit values
export const ignoreWithObjectKeys = Object.keys(
  import.meta.glob(['./modules/*.ts', '!**/index.ts']),
)
// todo: omit keys
export const ignoreWithObjectValues = Object.values(
  import.meta.glob(['./modules/*.ts', '!**/index.ts']),
)

export const namedEager = import.meta.glob<string>('./modules/*.ts', {
  eager: true,
  import: 'name',
})
// todo: omit values
// vite output: Object.keys({"./modules/a.ts": 0,"./modules/b.ts": 0,"./modules/index.ts": 0});
export const namedEagerWithObjectKeys = Object.keys(
  import.meta.glob<string>('./modules/*.ts', {
    eager: true,
    import: 'name',
  }),
)
// todo: omit keys
// vite output: Object.values([__vite_glob_11_0,__vite_glob_11_1,__vite_glob_11_2]);
export const namedEagerWithObjectValues = Object.values(
  import.meta.glob<string>('./modules/*.ts', {
    eager: true,
    import: 'name',
  }),
)

export const namedDefault = import.meta.glob<string>('./modules/*.ts', {
  import: 'default',
})
// todo: omit values
// vite output: Object.keys({"./modules/a.ts": 0,"./modules/b.ts": 0,"./modules/index.ts": 0});
export const namedDefaultWithObjectKeys = Object.keys(
  import.meta.glob<string>('./modules/*.ts', {
    import: 'default',
  }),
)
// todo: omit keys
// vite output: Object.values([() => import("./modules/a.ts").then(m => m["default"]),() => import("./modules/b.ts").then(m => m["default"]),() => import("./modules/index.ts").then(m => m["default"])]);
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

// todo: shouldn't contain './index.ts'
export const excludeSelf = import.meta.glob(
  './*.ts',
  // for test: annotation contain ")"
  /*
   * for test: annotation contain ")"
   * */
)

// unresolved import
// export const customQueryString = import.meta.glob('./*.ts', { query: 'custom' })

// unresolved import
// export const customQueryObject = import.meta.glob('./*.ts', {
//   query: {
//     foo: 'bar',
//     raw: true,
//   },
// })

export const parent = import.meta.glob('../../playground/src/*.ts', {
  query: '?url',
  import: 'default',
})

// unloadable dependency
// export const rootMixedRelative = import.meta.glob(
//   ['/*.ts', '../fixture-b/*.ts'],
//   { query: '?url', import: 'default' },
// )

export const cleverCwd1 = import.meta.glob(
  './node_modules/framework/**/*.page.js',
)

export const cleverCwd2 = import.meta.glob([
  './modules/*.ts',
  '../fixture-b/*.ts',
  '!**/index.ts',
])
