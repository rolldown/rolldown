import { dep } from './dep'

document.querySelector('#root').innerHTML = `
<div>
  <h1>Rebuild</h1>
  <input placeholder="test input" />
  <pre>[dep] ${dep}</pre>
</div>
`
