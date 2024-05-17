function DROP() {}
console.log((DROP(), DROP(), foo()))
console.log((DROP(), foo(), DROP()))
console.log((foo(), DROP(), DROP()))
for (DROP(); DROP(); DROP()) DROP();
DROP(), DROP(), foo();
DROP(), foo(), DROP();
foo(), DROP(), DROP();