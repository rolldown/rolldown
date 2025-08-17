import { expect } from 'vitest'
import { defineTest } from 'rolldown-tests'

export default defineTest({
  config: {
    input: ['main.js'],
    output: {
      sourcemap: true,
    },
  },
  afterTest: function (output) {
    expect(output.output[0].map).toBeDefined()
    expect(output.output[0].map!.toString()).toMatchInlineSnapshot(
      `"{"version":3,"file":"main.js","names":[],"sources":["../main.js"],"sourcesContent":["console.log(foo)\\n"],"mappings":";AAAA,QAAQ,IAAI"}"`,
    )
    expect(output.output[0].map!.toUrl()).toMatchInlineSnapshot(
      `"data:application/json;charset=utf-8;base64,eyJ2ZXJzaW9uIjozLCJmaWxlIjoibWFpbi5qcyIsIm5hbWVzIjpbXSwic291cmNlcyI6WyIuLi9tYWluLmpzIl0sInNvdXJjZXNDb250ZW50IjpbImNvbnNvbGUubG9nKGZvbylcbiJdLCJtYXBwaW5ncyI6IjtBQUFBLFFBQVEsSUFBSSJ9"`,
    )
    const map = output.output[0].map!
    map.file = 'main2.js'
    expect(map.toString()).toMatchInlineSnapshot(
      `"{"version":3,"file":"main2.js","names":[],"sources":["../main.js"],"sourcesContent":["console.log(foo)\\n"],"mappings":";AAAA,QAAQ,IAAI"}"`,
    )
    expect(map.toUrl()).toMatchInlineSnapshot(
      `"data:application/json;charset=utf-8;base64,eyJ2ZXJzaW9uIjozLCJmaWxlIjoibWFpbjIuanMiLCJuYW1lcyI6W10sInNvdXJjZXMiOlsiLi4vbWFpbi5qcyJdLCJzb3VyY2VzQ29udGVudCI6WyJjb25zb2xlLmxvZyhmb28pXG4iXSwibWFwcGluZ3MiOiI7QUFBQSxRQUFRLElBQUkifQ=="`,
    )
  },
})
