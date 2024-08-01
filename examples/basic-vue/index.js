const m = import.meta.glob('./dir/*.js', {
  // import: 'a',
  // eager: true
  query: {
    a: true,
    b: 'test',
    c: 10000
  }
})
//
console.log(`m: `, m)
