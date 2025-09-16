const modules1 = import.meta.glob(['./.dot/*.ts', './node_modules/*.js'])

const modules2 = import.meta.glob(['./.dot/*.ts', './node_modules/*.js'], { exhaustive: true })

export { modules1, modules2 }
