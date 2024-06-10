import('./lib').then(res => {
  console.log(`res.a: `, res.a)
  console.log(`res: `, res.d)
})
import { a as a2 } from './shared'
const a = 'index.js'
console.log(a, a2)
