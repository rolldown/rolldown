let hello = "world"
var world = "hello"
const greeting = "hi"

assert(hello === "world");
assert(world === "hello");
assert(greeting === "hi");

function second_level() {
  let secondLevelHello = "second level world"
  var secondLevelWorld = "second level hello"
  const secondLevelGreeting = "second level hi"

  assert(secondLevelHello === "second level world");
  assert(secondLevelWorld === "second level hello");
  assert(secondLevelGreeting === "second level hi");
}

second_level();