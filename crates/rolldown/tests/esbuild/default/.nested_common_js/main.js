function nestedScope() {
    const fn = require('./foo')
    console.log(fn())
}
nestedScope()