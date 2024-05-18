import * as ns from './empty.js'
export function foo() { return [ns, ns.missing] }
export function bar() { return [ns.missing] }