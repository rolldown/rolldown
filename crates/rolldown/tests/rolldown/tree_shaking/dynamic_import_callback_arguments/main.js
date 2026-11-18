export const arrowRest = () => import('./arrow-rest.js').then((...args) => args[0].value);
export const functionRest = () =>
  import('./function-rest.js').then(function (...args) {
    return args[0].value;
  });
export const functionArguments = () =>
  import('./arguments.js').then(function () {
    return arguments[0].value;
  });
export const namedArguments = () =>
  import('./named-arguments.js').then(function (ns) {
    return ns.value + arguments[0].other;
  });
export const nestedArrow = () =>
  import('./nested-arrow.js').then(function () {
    const read = () => arguments[0].value;
    return read();
  });
export const unusedArguments = () =>
  import('./unused.js').then(function () {
    function inner() {
      return arguments[0];
    }
    return inner('expected');
  });
export const unusedRest = () => import('./unused-rest.js').then((...args) => 'expected');
export const trailingRest = () => import('./trailing-rest.js').then((ns, ...args) => ns.value);
export const destructuredRest = () => import('./destructured-rest.js').then((...[ns]) => ns.value);
export function inheritedArguments(value) {
  return import('./inherited-arguments.js').then(() => arguments[0]);
}
export const rejectionArguments = () =>
  import('./rejection-arguments.js').then(
    function () {
      return 'expected';
    },
    function () {
      return arguments[0];
    },
  );
