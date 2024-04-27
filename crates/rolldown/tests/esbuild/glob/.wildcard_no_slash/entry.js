const ab = Math.random() < 0.5 ? 'a.js' : 'b.js'
console.log({
  concat: {
    require: require('./src/file-' + ab + '.js'),
    import: import('./src/file-' + ab + '.js'),
  },
  template: {
    require: require(`./src/file-${ab}.js`),
    import: import(`./src/file-${ab}.js`),
  },
})