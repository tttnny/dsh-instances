<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Message } from '@arco-design/web-vue'
import { api } from '@/api'
import { useLauncherStore } from '@/stores/launcher'

const props = defineProps<{ visible: boolean; presetVersion?: string | null }>()
const emit = defineEmits<{ (e: 'update:visible', v: boolean): void; (e: 'created'): void }>()

const { t } = useI18n()
const store = useLauncherStore()

const DEDICATED = '__dedicated__'

const instanceName = ref('')
const version = ref<string>('')
const homeId = ref<string | undefined>(DEDICATED)
const defaultProfile = ref<string | undefined>(undefined)
const profiles = ref<string[]>([])
const dedicatedPath = ref('')
const busy = ref(false)

const dedicated = computed(() => homeId.value === DEDICATED)
const installedVersion = computed(() => store.versions.find((v) => v.version === version.value))
const isSourceBuild = computed(
  () =>
    !installedVersion.value &&
    store.remoteVersions.some((v) => v.version === version.value && v.source === 'github'),
)

function suggestName(base: string): string {
  let candidate = base || 'instance'
  let n = 2
  while (store.instances.some((i) => i.name === candidate)) {
    candidate = `${base}-${n}`
    n += 1
  }
  return candidate
}

async function refreshDedicatedPath() {
  if (!dedicated.value) return
  dedicatedPath.value = await api.defaultDedicatedHomePath(
    instanceName.value.trim() || version.value || 'instance',
  )
}

async function refreshProfiles() {
  profiles.value = []
  if (!homeId.value || dedicated.value) {
    defaultProfile.value = undefined
    return
  }
  try {
    profiles.value = await api.listProfiles(homeId.value)
    if (defaultProfile.value && !profiles.value.includes(defaultProfile.value)) {
      defaultProfile.value = undefined
    }
  } catch (e) {
    Message.error(String(e))
  }
}

watch(
  () => props.visible,
  async (v) => {
    if (!v) return
    if (store.remoteVersions.length === 0) {
      await store.refreshRemoteVersions()
    }
    const firstVersion = props.presetVersion ?? store.remoteVersions[0]?.version ?? store.versions[0]?.version ?? ''
    version.value = firstVersion
    instanceName.value = suggestName(firstVersion)
    homeId.value = DEDICATED
    defaultProfile.value = undefined
    await refreshDedicatedPath()
  },
)

watch([homeId, instanceName, version], async () => {
  await refreshDedicatedPath()
})
watch(homeId, refreshProfiles)

const canConfirm = computed(
  () =>
    !busy.value &&
    instanceName.value.trim().length > 0 &&
    !!version.value &&
    !!homeId.value &&
    !store.instances.some((i) => i.name === instanceName.value.trim()) &&
    !store.instanceNameBusy(instanceName.value.trim()),
)

function close() {
  emit('update:visible', false)
}

async function onConfirm() {
  if (!canConfirm.value) return
  busy.value = true
  try {
    await api.startCreateInstanceTask(
      instanceName.value.trim(),
      version.value,
      dedicated.value ? null : homeId.value!,
      dedicated.value,
    )
    await store.refreshTasks()
    Message.success(t('newInstance.taskAdded'))
    emit('created')
    close()
  } catch (e) {
    Message.error(String(e))
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <a-modal
    :visible="visible"
    :title="t('newInstance.title')"
    :ok-text="t('newInstance.create')"
    :cancel-text="t('common.cancel')"
    :ok-button-props="{ disabled: !canConfirm, loading: busy }"
    @ok="onConfirm"
    @cancel="close"
  >
    <a-form layout="vertical" :model="{}">
      <a-form-item :label="t('newInstance.name')" required>
        <a-input v-model="instanceName" :placeholder="t('newInstance.namePlaceholder')" />
      </a-form-item>
      <a-form-item :label="t('newInstance.version')" required>
        <a-select v-model="version" :placeholder="t('common.loading')">
          <a-option v-for="v in store.remoteVersions" :key="v.version" :value="v.version">
            {{ v.version }}
          </a-option>
          <a-option
            v-for="v in store.versions.filter((x) => !store.remoteVersions.some((r) => r.version === x.version))"
            :key="v.version"
            :value="v.version"
          >
            {{ v.version }}
          </a-option>
        </a-select>
        <a-alert v-if="isSourceBuild" type="warning" class="dialog-hint">
          {{ t('newInstance.sourceBuildHint') }}
        </a-alert>
        <a-alert v-else-if="installedVersion" type="info" class="dialog-hint">
          {{ t('newInstance.alreadyInstalled') }}
        </a-alert>
        <a-alert v-else-if="version" type="info" class="dialog-hint">
          {{ t('newInstance.willInstall', { version }) }}
        </a-alert>
      </a-form-item>
      <a-form-item :label="t('newInstance.home')" required>
        <a-select v-model="homeId">
          <a-option :value="DEDICATED">{{ t('newInstance.dedicatedHome') }}</a-option>
          <a-option v-for="h in store.homes" :key="h.id" :value="h.id">
            {{ h.name }}（{{ h.path }}）
          </a-option>
        </a-select>
        <a-alert v-if="dedicated" type="info" class="dialog-hint">
          {{ t('newInstance.dedicatedHomeHint', { path: dedicatedPath }) }}
        </a-alert>
      </a-form-item>
      <a-form-item v-if="!dedicated" :label="t('newInstance.defaultProfile')">
        <a-select
          v-model="defaultProfile"
          :placeholder="t('newInstance.defaultProfilePlaceholder')"
          allow-clear
        >
          <a-option v-for="p in profiles" :key="p" :value="p">{{ p }}</a-option>
        </a-select>
      </a-form-item>
    </a-form>
  </a-modal>
</template>

<style lang="scss" scoped>
.dialog-hint {
  margin-top: 8px;
}
</style>
