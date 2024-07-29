// Ported from https://github.com/vitejs/vite/blob/main/packages/vite/src/node/plugins/dynamicImportVars.ts#L42-L65
export default (
  glob,
  path,
) => {
  const v = glob[path]
  if (v) {
    return typeof v === 'function' ? v() : Promise.resolve(v)
  }
  return new Promise((_, reject) => {
    ;(typeof queueMicrotask === 'function' ? queueMicrotask : setTimeout)(
      // TODO: Better error message like https://github.com/vitejs/vite/pull/16519
      reject.bind(null, new Error('Unknown variable dynamic import: ' + path)),
    )
  })
}
