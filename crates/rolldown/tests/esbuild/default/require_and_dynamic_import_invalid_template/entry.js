require(tag`./b`)
require(`./${b}`)

try {
    require(tag`./b`)
    require(`./${b}`)
} catch {
}

(async () => {
    import(tag`./b`)
    import(`./${b}`)
    await import(tag`./b`)
    await import(`./${b}`)

    try {
        import(tag`./b`)
        import(`./${b}`)
        await import(tag`./b`)
        await import(`./${b}`)
    } catch {
    }
})()
