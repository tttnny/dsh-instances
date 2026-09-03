<script setup lang="ts">
import { computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useLauncherStore } from '@/stores/launcher'

const route = useRoute()
const router = useRouter()
const { t } = useI18n()
const store = useLauncherStore()

type Flow = 'create' | 'plugins'

const flow = computed<Flow>(() => {
  const name = route.name as string
  return name === 'download-plugins' || name === 'plugin-version' || name === 'plugin-install'
    ? 'plugins'
    : 'create'
})

const stepTitles = computed(() =>
  flow.value === 'create'
    ? [t('download.stepPickVersion'), t('download.stepConfig')]
    : [t('plugins.stepPickPlugin'), t('plugins.stepPickVersion'), t('plugins.stepPickTarget')],
)

const current = computed(() => {
  const name = route.name as string
  if (flow.value === 'create') return name === 'download-name' ? 2 : 1
  if (name === 'plugin-version') return 2
  if (name === 'plugin-install') return 3
  return 1
})

/** Clicking a finished step goes back; forward jumps stay disabled by flow state. */
function onStepChange(next: number) {
  if (next >= current.value) return
  if (flow.value === 'create') {
    if (next === 1) router.push({ name: 'download-create' })
    return
  }
  if (next === 1) {
    router.push({ name: 'download-plugins' })
  } else if (next === 2 && store.pluginWizard) {
    router.push({ name: 'plugin-version' })
  }
}
</script>

<template>
  <div class="dl-page download-page">
    <div class="dl-card wizard-head">
      <a-steps :current="current" small @change="onStepChange">
        <a-step v-for="(title, i) in stepTitles" :key="title + i" :title="title" />
      </a-steps>
    </div>
    <div class="wizard-body">
      <router-view />
    </div>
  </div>
</template>

<style lang="scss" scoped>
.download-page {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.wizard-head {
  padding: 16px 24px;
}

.wizard-body {
  min-height: 0;
}
</style>
