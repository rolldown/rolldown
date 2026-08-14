export class Foo {}
export function foo() {}

export const baz = function () {};

function __name() {
  // Prevent removal by oxc minifier
  console.log();
}
// rolldown to deconflict `__name` function
__name();
