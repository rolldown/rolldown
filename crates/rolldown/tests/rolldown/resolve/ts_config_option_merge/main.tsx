import {add} from '@/util'

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
console.log(`add(1, 2): `, add(1, 2))
