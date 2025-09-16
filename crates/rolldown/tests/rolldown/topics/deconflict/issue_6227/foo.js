if (true) {
  const foo = 'foo' + globalThis.foo
  setTimeout(() => {
    globalThis.array.push(foo)
  }, 100)
}
