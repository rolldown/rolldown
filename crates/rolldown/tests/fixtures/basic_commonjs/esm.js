export default function esm_default_fn() {}
export const esm_named_var = 1;
export function esm_named_fn() {}
export class esm_named_class {}

const hoisted_var = 1;
function hoisted_fn() {
    const bar = 1 // shouldn't hoisted
}
class hoisted_class {}