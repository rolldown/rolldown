const m1 = import.meta.glob('./dir/*', { eager: true })

const m2 = import.meta.glob('/src/dir/*', { eager: true })

const m3 = import.meta.glob('./dir/*.js', { eager: true })

export { m1, m2, m3 }
