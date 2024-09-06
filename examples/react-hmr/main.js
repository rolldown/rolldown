// TODO plugin-react-refresh insert it at entry load
import 'react-refresh-entry.js'
import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App.jsx'

ReactDOM.createRoot(document.getElementById('app')).render(
  React.createElement(App),
)
