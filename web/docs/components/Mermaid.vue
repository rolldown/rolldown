<template>
  <div v-html="svg"></div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useData } from 'vitepress'

const { isDark } = useData()

const props = defineProps<{
  graph: string
  id: string
}>()

const svg = ref<string>('')

onMounted(async () => {
  const { default: mermaid } = await import('mermaid')
  mermaid.initialize({
    securityLevel: 'loose',
    startOnLoad: false,
    theme: isDark.value ? 'dark' : 'default',
  })

  const render = await mermaid.render(props.id, decodeURIComponent(props.graph))
  svg.value = render.svg
})
</script>
