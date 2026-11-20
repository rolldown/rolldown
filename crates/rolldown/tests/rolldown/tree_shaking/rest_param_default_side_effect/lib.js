export function restFunction(...[value = globalThis.calls.push('function')]) {}

export const restArrow = (...[value = globalThis.calls.push('arrow')]) => {};

export const restFunctionExpression = function (
  ...[value = globalThis.calls.push('function expression')]
) {};

export function restObjectPattern(...{ value = globalThis.calls.push('object pattern') }) {}

export function plainRest(...rest) {}
