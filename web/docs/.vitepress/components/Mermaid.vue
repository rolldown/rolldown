<template>
  <div>
    <div v-if="isDark" v-html="svgDark"></div>
    <div v-else v-html="svgLight"></div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import { useData } from 'vitepress'
import { type MermaidConfig } from 'mermaid'
import mermaid from 'mermaid'

const render = async (
  id: string,
  code: string,
  config: MermaidConfig,
): Promise<string> => {
  mermaid.initialize(config)
  const { svg } = await mermaid.render(id, code)
  return svg
}

const { isDark } = useData()

const props = defineProps({
  graph: {
    type: String,
    required: true,
  },
  id: {
    type: String,
    required: true,
  },
})

const svgLight = ref<string>('')
const svgDark = ref<string>('')

const renderChart = async () => {
  const mermaidConfig = {
    securityLevel: 'loose',
    startOnLoad: false,
  }

  svgLight.value = await render(props.id, decodeURIComponent(props.graph), {
    ...mermaidConfig,
    theme: 'default',
  })
  svgDark.value = await render(props.id, decodeURIComponent(props.graph), {
    ...mermaidConfig,
    theme: 'dark',
  })
}

watch(
  [() => props.graph],
  () => {
    renderChart()
  },
  { immediate: true },
)
</script>
