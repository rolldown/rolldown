---
outline: false
---

<script setup>

const contributors = [
  ['Hana', 'https://github.com/h-a-n-a'],
  ['Kui Li (underfin)', 'https://github.com/underfin'],
].sort((a, b) => a[0].localeCompare(b[0])); // Sort alphabetically by name

</script>

# Acknowledgements

The Rolldown project was originally created by [Yinan Long](https://github.com/Brooooooklyn) (aka Brooooooklyn, author of [NAPI-RS](https://napi.rs/)). Today, Rolldown is led by [Evan You](https://github.com/yyx990803) (the creator of [Vite](https://vitejs.dev/)) together with a full-time [team](./team.md) and passionate open source [contributors](https://github.com/rolldown/rolldown/graphs/contributors).

## Past contributors

We’d like to recognize a few people who are former team members or have made significant contributions to the project, documentation, and its ecosystem (listed in alphabetical order):

<ul>
<template v-for="contributor in contributors" :key="contributor[0]">
  <li>
    <a :href="contributor[1]" target="_blank">
      {{ contributor[0] }}
    </a>
  </li>
</template>
</ul>

This list is not exhaustive.

## Additional Thanks

Additionally, we’re grateful to:

- [Charlike Mike Reagent](https://github.com/tunnckoCore) for letting us use the `rolldown` package name on npm
