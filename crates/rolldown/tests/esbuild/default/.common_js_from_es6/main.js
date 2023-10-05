const {foo} = require('./foo')
console.log(foo(), bar())
const {bar} = require('./bar') // This should not be hoisted