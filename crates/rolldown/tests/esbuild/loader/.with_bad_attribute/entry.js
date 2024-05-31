import foo from './foo.json' with { '': 'json' }
import bar from './foo.json' with { garbage: 'json' }
console.log(bar)