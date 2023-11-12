<script setup lang="ts">
import CodeBlock from './CodeBlock.vue';
import { defineProps, onMounted, ref } from 'vue'

const props = defineProps({
  title: {
    type: String,
    required: true
  },
  code: {
    type: String,
    required: true
  },
  autoFocus: {
    type: Boolean,
    required: false
  }
})

const input = ref(null)

onMounted(() => {
  const [basename, _] = props.title.split('.')
  // if (props.autoFocus && input.value) {
  //   
  //   const target = input.value as any;
  //   target.contentEditable = true;
  //   var range = document.createRange();
  //   
  //   // Select the text nodes within the div (startNode, startOffset, endNode, endOffset)
  //   range.setStart(target.childNodes[0], 0);
  //   range.setEnd(target.childNodes[0], basename.length);
  //   getSelection()?.removeAllRanges();
  //
  //   // Add the new range to the selection
  //   getSelection()?.addRange(range);
  //   target.focus();
  // }
})

</script>

<template>
  <div>
    <div class="title" ref="input" contenteditable @input="$emit('title', $event)">{{ title }}</div>
    <CodeBlock :code="code" @code="$emit('code', $event)" />
  </div>
</template>

<style>
.title {
  margin: auto;
}
</style>
