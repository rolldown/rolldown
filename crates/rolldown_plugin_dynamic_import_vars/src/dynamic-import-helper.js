// Ported from https://github.com/vitejs/vite/blob/main/packages/vite/src/node/plugins/dynamicImportVars.ts#L42-L65
export default (
  glob,
  path,
  segments,
) => {
  const v = glob[path] ?? glob['./' + path];
  if (v) {
    return typeof v === 'function' ? v() : Promise.resolve(v);
  }
  return new Promise((_, reject) => {
    (typeof queueMicrotask === 'function' ? queueMicrotask : setTimeout)(
      reject.bind(
        null,
        new Error(
          'Unknown variable dynamic import: ' +
            path +
            (path.split('/').length !== segments
              ? '. Note that variables only represent file names one level deep.'
              : ''),
        ),
      ),
    );
  });
};
