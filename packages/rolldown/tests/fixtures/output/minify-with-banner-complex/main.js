// This is a regular comment that should be removed by minification
function hello() {
  // Another comment
  const message = 'hello world';
  console.log(message);
  
  /* Block comment */
  const unused = 'this should be removed by DCE';
}

function world() {
  console.log('world');
}

hello();
world();
