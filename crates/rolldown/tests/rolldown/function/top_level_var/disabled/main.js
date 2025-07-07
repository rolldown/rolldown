let firstLevelLet = "let"
var firstLevelVar = "var"
const firstLevelConst = "const"
class FirstLevelClass {}
console.log(firstLevelLet, firstLevelVar, firstLevelConst, new FirstLevelClass());

if (true) {
  let shouldNotBeSubstitutedLet = "let";
  console.log(shouldNotBeSubstitutedLet)
}

function second_level() {
    let secondLevelLet = "let";
    var secondLevelVar = "var";
    const secondLevelConst = "const";
    class SecondLevelClass {}

    console.log(secondLevelLet, secondLevelVar, secondLevelConst, new SecondLevelClass());
}

second_level();
