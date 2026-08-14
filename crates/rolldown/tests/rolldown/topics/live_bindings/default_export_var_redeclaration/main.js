// Test case for var redeclaration with export default
// The symbol should not be reused because of redeclaration
var foo = 42;
var foo = 41;

export default foo;
