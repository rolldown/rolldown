<script setup lang="ts">
import DefaultTheme from 'vitepress/theme'
import { inBrowser, useData, useRouter } from 'vitepress'
import { watch } from 'vue'

const { page } = useData()
const { go } = useRouter()

// Ref: https://github.com/vuejs/vitepress/issues/4160#issuecomment-2308509400
// > brc-dd: Don't use this if your application is SEO-critical as the page will still be returned with a 404 status code.
// hyf0: We're redirecting old paths to new ones, so SEO for those old paths is not critical. We actually want them to be 404 in search engines.

// WARNING: This is only used to redirect removed documentation pages to their new locations. Don't rediect existing pages with this!
const redirects = Object.entries({
  '/reference/': '/apis/' // TODO: remove this after publishing next version of rolldown
})

watch(
  () => page.value.isNotFound,
  (isNotFound) => {
    if (!isNotFound || !inBrowser) return
    const redirect = redirects.find(([from]) => window.location.pathname.startsWith(from))
    if (!redirect) return
    go(redirect[1] + window.location.pathname.slice(redirect[0].length) + window.location.search + window.location.hash)
  },
  { immediate: true }
)
</script>

<template>
  <DefaultTheme.Layout />
</template>