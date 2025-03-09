let count = 0
export let createApp = function createApp() {
  const container = document.createElement('div')
  const countDiv = document.createElement('div')
  countDiv.innerHTML = `Count: ${count};`

  const incBtn = document.createElement('button')
  incBtn.innerHTML = 'Click me to increase count222'

  incBtn.onclick = function () {
    count++
    countDiv.innerHTML = `Count: ${count};`
  } // onclick event is bind to the original printMe function

  container.appendChild(countDiv)
  container.appendChild(incBtn)

  return container
}

if (import.meta.hot) {
  console.log('🔥Hot Module Replacement enabled')
  import.meta.hot.accept(function (newExports) {
    createApp = newExports.createApp
    globalThis.render()
    console.log('🔥Updated')
  })
}

// exports.createApp = createApp
