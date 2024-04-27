const ab = Math.random() < 0.5 ? 'a.js' : 'b.js'
console.log({
  concat: {
    require: require('./src/' + ab + '.js'),
    import: import('./src/' + ab + '.js'),
  },
  template: {
    require: require(`./src/${ab}.js`),
    import: import(`./src/${ab}.js`),
  },
})