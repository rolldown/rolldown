const m = import.meta.glob('./dir/*.js', {
  query: {
    a: true,
    b: 'test',
    c: 10000,
  },
})

export { m }
