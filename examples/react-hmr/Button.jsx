import { useState } from 'react'

export default function Button() {
  const [count, setCount] = useState(0)
  return (
    <p>
      <button id="state-button" onClick={() => setCount((count) => count + 1)}>
        count is: {count}12332123521
      </button>
    </p>
  )
}
