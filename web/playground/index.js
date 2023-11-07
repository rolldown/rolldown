import init, { bundle, FileItem, AssetItem } from '../wasm'
await init();
let res = bundle([new FileItem('/index.js', 'const a = 3')])
let normalizedRes = res.map((item) => {
  return {
    name: item.name,
    content: item.content,
  }
})

console.log(normalizedRes)
