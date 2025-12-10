import foo from "./foo.thing" with { type: 'whatever' }
import bar from "./bar.thing" with { whatever: 'true' }
console.log(foo, bar)