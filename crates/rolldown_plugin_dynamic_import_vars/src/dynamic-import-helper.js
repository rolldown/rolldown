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
