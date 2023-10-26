import def, {foo} from 'external'
console.log(def(), foo())
console.log(new def(), new foo())