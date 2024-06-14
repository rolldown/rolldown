import('./lib').then(res => {
  console.log(`res.a: `, res.a)
})
// import('./lib').then(res => {
//   console.log(`res.a: `, res.d)
// })
import { a as a2 } from './shared'
const a = 'index.js'
console.log(a, a2)

