import 'ext-bare' with { custom: 'a"b', 'k"ey': 'c\\d' };
import { x } from 'ext-named' with { custom: 'a"b' };
export * from 'ext-star' with { custom: 'a"b' };
console.log(x);
