export class Foo {}
export function foo() {}

export const baz = function() {

}

function __name() {}
// rolldown to deconflict `__name` function
__name();
