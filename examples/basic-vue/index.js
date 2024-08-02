const m = import.meta.glob('./dir/*.js', {
  // import: 'a',
  // eager: true
  query: "?raw"
})
//
console.log(`m: `, m)
