<script setup lang="ts" >
import { ref, onMounted, Ref } from 'vue';
import ModuleBlock from './components/ModuleBlock.vue';
import init, { bundle } from '../../wasm'
import { convertAssetListToModuleList, normalizeModules, uniqueModulePath } from './utils/index'
import type { ModuleInfo } from './utils/index'

const moduleList: Ref<ModuleInfo[]> = ref([
  { title: "index.js", code: `console.log("hello world")` }
])

const outputs: Ref<ModuleInfo[]> = ref([])

const wasmLoadFinished = ref(false)

onMounted(() => {
  init().then(_ => {
    wasmLoadFinished.value = true;
  })
})


const handleBuild = () => {
  const fileList = normalizeModules(moduleList.value)
  let res = bundle(fileList)
  outputs.value = convertAssetListToModuleList(res)
}

const handleAddModule = () => {
  const title = uniqueModulePath(moduleList.value)
  moduleList.value.push({
    title,
    code: `console.log("hello world")`,
    autofocus: true
  });
}

</script>

<template>
  <div class="container">
    <!-- module declaration block -->
    <div class="module-list column">
      <ModuleBlock v-for="item in moduleList" :code="item.code" :title="item.title" @code="item.code = $event"
        @title="item.title = $event.target.innerText" :auto-focus="item.autofocus" />
      <button @click="handleAddModule">Add module</button>
    </div>
    <!-- output block -->
    <div class="output column">
      <button @click="handleBuild" :disabled="!wasmLoadFinished">build</button>
      <ModuleBlock v-for="item in outputs" :code="item.code" :title="item.title" @code="item.code = $event"
        @title="item.title = $event.target.innerText" />
    </div>
  </div>
</template>

<style scoped>
.container {
  display: flex;
}

.column {
  flex: 1;
}
</style>
