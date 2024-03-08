// These shouldn't warn since the format is CommonJS
console.log(Object.keys(require.cache))
console.log(Object.keys(require.extensions))
delete require.cache['fs']
delete require.extensions['.json']