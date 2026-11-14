import 'ext-bare' with { custom: 'a"b', 'k"ey': 'c\\d' };
import { x } from 'ext-named' with { custom: 'a"b' };
export * from 'ext-star' with { custom: 'a"b' };
import 'ext-surrogate' with { custom: 'a\uD800', 'k\uD800': 'b' };
console.log(x);
