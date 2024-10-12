import foo from './foo.json' with { type: '' }
import bar from './foo.json' with { type: 'garbage' }
console.log(bar)