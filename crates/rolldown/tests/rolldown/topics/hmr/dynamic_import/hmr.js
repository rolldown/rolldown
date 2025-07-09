import.meta.hot.accept((mod) => {
  if (mod) {
    console.log('.hmr', mod.foo)
  }
})
