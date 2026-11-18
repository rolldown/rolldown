export const values = [
  require('data:text/javascript,export default "single\'quote";').default,
  require("data:text/javascript,export default 'double\"quote';").default,
  require('data:text/javascript,export default "back\\\\slash";').default,
  require('data:text/javascript,\nexport default "line-break";\r').default,
];
