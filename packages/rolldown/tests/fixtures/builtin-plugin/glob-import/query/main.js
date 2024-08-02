const m1 = import.meta.glob('./dir/*.js', {
  query: '?raw',
})

export { m1 }
