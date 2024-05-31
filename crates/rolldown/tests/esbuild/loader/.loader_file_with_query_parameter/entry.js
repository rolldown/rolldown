// Each of these should have a separate identity (i.e. end up in the output file twice)
import foo from './file.txt?foo'
import bar from './file.txt?bar'
console.log(foo, bar)