const modules1 = import.meta.glob('./dir/*.{js,ts}')

const modules2 = import.meta.glob('./dir/*.{js,ts}', { import: 'value' })

export { modules1, modules2 }
