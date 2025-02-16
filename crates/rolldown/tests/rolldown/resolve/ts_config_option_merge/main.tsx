function first() {
  return function (...args: any[]) {}
}

class Foo {
  @first()
  method(@first() test: string) {
    return test
  }
}



console.log(<Div/>)
