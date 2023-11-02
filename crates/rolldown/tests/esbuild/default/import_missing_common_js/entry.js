import fn, {x as a, y as b} from './foo'
if (false) {  // the test case is error case, so we don't need to run it
    console.log(fn(a, b))
}
