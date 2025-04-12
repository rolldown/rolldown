const modules1 = import.meta.glob('./dir/*.js', { eager: true })

const modules2 = import.meta.glob('./dir/*.js', {
  import: 'value',
  eager: true,
})

const modules3 = import.meta.glob('./dir/*.js', { import: '*', eager: true })

const modules4 = import.meta.glob('./dir/*.js', {
  import: 'default',
  eager: true,
})

export { modules1, modules2, modules3, modules4 }
