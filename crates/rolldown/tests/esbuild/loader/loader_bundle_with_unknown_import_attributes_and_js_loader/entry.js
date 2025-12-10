import foo from "./foo.js" with { type: 'js' }
import bar from "./bar.js" with { js: 'true' }
import foo2 from "data:text/javascript,foo" with { type: 'js' }
import bar2 from "data:text/javascript,bar" with { js: 'true' }
console.log(foo, bar, foo2, bar2)