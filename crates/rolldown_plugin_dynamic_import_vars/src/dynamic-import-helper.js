export default (
  glob,
  path,
  segments,
) => {
  const query = path.lastIndexOf('?');
  const v = glob[
    query === -1 || query < path.lastIndexOf('/')
      ? path
      : path.slice(0, query)
  ];
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
