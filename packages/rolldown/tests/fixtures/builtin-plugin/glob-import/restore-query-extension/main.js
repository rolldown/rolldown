const m1 = import.meta.glob('./dir/*.js', {
  query: {
    a: 1000,
    b: 'test',
  },
})

export { m1 }
