import React from 'react'
import Button from './Button'

function App() {
  return (
    <div className="App">
      <header className="App-header">
        <h1>Hello Rolldown + React</h1>
        <Button />
        <p>
          Edit <code>App.jsx</code> and save to test HMR updates.
        </p>
        <a
          className="App-link"
          href="https://reactjs.org"
          target="_blank"
          rel="noopener noreferrer"
        >
          Learn React
        </a>
      </header>
    </div>
  )
}

export default App
