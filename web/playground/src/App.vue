<script setup lang="ts" >
import { ref } from 'vue';
import ModuleBlock from './components/ModuleBlock.vue';

const moduleList = ref([
  { title: "main.js", code: `console.log("hello world")` }
])

setTimeout(() => {
  console.log(moduleList.value)
}, 2000)

</script>

<template>
  <div>
    <ModuleBlock v-for="item in moduleList" :code="item.code" :title="item.title" @code="item.code = $event"
      @title="item.title = $event.target.innerText" />
  </div>
</template>

<style scoped>
.logo {
  height: 6em;
  padding: 1.5em;
  will-change: filter;
  transition: filter 300ms;
}

.logo:hover {
  filter: drop-shadow(0 0 2em #646cffaa);
}

.logo.vue:hover {
  filter: drop-shadow(0 0 2em #42b883aa);
}
</style>
