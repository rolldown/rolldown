import { bundle, FileItem, AssetItem } from 'rolldown-wasm'

let res = bundle([new FileItem('/index.js', 'const a = 3')])
let normalizedRes = res.map((item) => {
  return {
    name: item.name,
    content: item.content,
  }
})

console.log(normalizedRes)
