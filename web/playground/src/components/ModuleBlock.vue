<script setup lang="ts">
import CodeBlock from './CodeBlock.vue';
import { defineProps } from 'vue'

defineProps({
  title: String,
  code: String
})

</script>

<template>
  <div>
    <div contenteditable @input="$emit('title', $event)">{{ title }}</div>
    <CodeBlock :code="code" @code="$emit('code', $event)"/>
  </div>
</template>
