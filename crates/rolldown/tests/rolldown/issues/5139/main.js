import { Foo as Bar, foo as bar, baz as b } from "./foo.js";

export class Foo {}
export function foo() {}
export const baz = function() {};

export { Bar, bar, b };
