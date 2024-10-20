class T {}

T.prototype.valueOf = function () {
  console.log('test')
}

let t = new T()

t + 1
