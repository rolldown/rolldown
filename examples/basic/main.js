const modules1 = import.meta.glob('./dir/*.js')

const modules2 = import.meta.glob('./dir/*.js', { import: 'value' })

export { modules1, modules2 }
