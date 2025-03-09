import { createApp } from './com'

function render() {
  while (document.body.firstChild) {
    document.body.removeChild(document.body.firstChild)
  }
  const app = createApp() // Store the element to re-render on print.js changes
  console.log('app', app, createApp)
  document.body.appendChild(app)
}
globalThis.render = render

render()

// exports.createApp = createApp
