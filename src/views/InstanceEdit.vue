<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { Message } from '@arco-design/web-vue'
import { api } from '@/api'
import { useLauncherStore } from '@/stores/launcher'
import type { DshInstance } from '@/api/types'

const route = useRoute()
const router = useRouter()
const { t } = useI18n()
const store = useLauncherStore()

const editingId = computed(() => (route.params.id as string | undefined) ?? null)

const name = ref('')
const versionId = ref<string | undefined>(undefined)
const DEDICATED = '__dedicated__'
const homeId = ref<string | undefined>(undefined)
const dedicatedPath = ref('')
const defaultProfile = ref<string | undefined>(undefined)
const profiles = ref<string[]>([])
const saving = ref(false)

// --- Web port ---------------------------------------------------------------

const portInput = ref('')
const portBusy = ref(false)

function parsePortInput(raw: string): number | null {
  const text = raw.trim()
  if (!text) return null
  const n = Number(text)
  return Number.isInteger(n) && n >= 1 && n <= 65535 ? n : null
}

async function applyPort() {
  if (!editingId.value) return
  portBusy.value = true
  try {
    const updated = await api.setInstancePort(editingId.value, parsePortInput(portInput.value))
    const inst = store.instanceById(editingId.value)
    if (inst) inst.port = updated.port ?? null
    portInput.value = updated.port ? String(updated.port) : ''
    Message.success(
      updated.port
        ? t('instanceEdit.portSaved', { port: updated.port })
        : t('instanceEdit.portSavedRandom'),
    )
  } catch (e) {
    Message.error(String(e))
  } finally {
    portBusy.value = false
  }
}

interface EnvRow {
  key: string
  value: string
}
const envRows = ref<EnvRow[]>([])

const ENV_KEY_RE = /^[A-Za-z_][A-Za-z0-9_]*$/
const RESERVED_KEYS = new Set(['DSH_HOME'])

function envKeyError(row: EnvRow): string | null {
  if (!row.key) return null
  if (RESERVED_KEYS.has(row.key)) return t('instanceEdit.envKeyReserved')
  if (!ENV_KEY_RE.test(row.key)) return t('instanceEdit.envKeyInvalid')
  return null
}

const envValid = computed(() => envRows.value.every((r) => !envKeyError(r)))

onMounted(async () => {
  if (!editingId.value) return
  const inst = store.instanceById(editingId.value) ?? (await api.listInstances()).find((i) => i.id === editingId.value)
  if (!inst) {
    Message.error(t('instanceEdit.notFound'))
    router.replace({ name: 'home' })
    return
  }
  name.value = inst.name
  versionId.value = inst.version_id
  homeId.value = inst.home_id
  defaultProfile.value = inst.default_profile ?? undefined
  portInput.value = inst.port ? String(inst.port) : ''
  envRows.value = Object.entries(inst.env_overrides).map(([key, value]) => ({ key, value }))
  await loadIcon()
})

// --- Instance icon ------------------------------------------------------------

const iconUrl = ref<string | null>(null)
const iconInput = ref('')
const iconBusy = ref(false)

async function loadIcon() {
  if (!editingId.value) return
  try {
    iconUrl.value = await api.readInstanceIcon(editingId.value)
  } catch {
    iconUrl.value = null
  }
}

async function applyIconInput() {
  if (!editingId.value || !iconInput.value.trim()) return
  iconBusy.value = true
  try {
    await api.setInstanceIcon(editingId.value, iconInput.value.trim())
    iconInput.value = ''
    await loadIcon()
    await store.refreshInstances()
    Message.success(t('instanceEdit.iconUpdated'))
  } catch (e) {
    Message.error(String(e))
  } finally {
    iconBusy.value = false
  }
}

async function pickIconFile() {
  if (!editingId.value) return
  const { open } = await import('@tauri-apps/plugin-dialog')
  const file = await open({
    multiple: false,
    filters: [{ name: 'Image', extensions: ['png', 'jpg', 'jpeg', 'webp', 'gif', 'bmp'] }],
  })
  if (typeof file !== 'string') return
  iconBusy.value = true
  try {
    await api.setInstanceIcon(editingId.value, file)
    await loadIcon()
    await store.refreshInstances()
    Message.success(t('instanceEdit.iconUpdated'))
  } catch (e) {
    Message.error(String(e))
  } finally {
    iconBusy.value = false
  }
}

async function clearIcon() {
  if (!editingId.value) return
  try {
    await api.clearInstanceIcon(editingId.value)
    await loadIcon()
    await store.refreshInstances()
  } catch (e) {
    Message.error(String(e))
  }
}

watch(homeId, async (v) => {
  profiles.value = []
  if (v === DEDICATED) {
    dedicatedPath.value = await api.defaultDedicatedHomePath(name.value.trim() || 'instance')
    return
  }
  if (!v) return
  try {
    profiles.value = await api.listProfiles(v)
    if (defaultProfile.value && !profiles.value.includes(defaultProfile.value)) {
      defaultProfile.value = undefined
    }
  } catch (e) {
    Message.error(String(e))
  }
})

watch(name, async (v) => {
  if (homeId.value === DEDICATED) {
    dedicatedPath.value = await api.defaultDedicatedHomePath(v.trim() || 'instance')
  }
})

// --- Open directory / view log -------------------------------------------------

const dirBusy = ref(false)
const logBusy = ref(false)

async function onOpenDirectory() {
  if (!editingId.value) return
  dirBusy.value = true
  try {
    const path = await api.openInstanceDirectory(editingId.value)
    Message.success(t('instanceEdit.dirOpened', { path }))
  } catch (e) {
    Message.error(String(e))
  } finally {
    dirBusy.value = false
  }
}

async function onViewLog() {
  if (!editingId.value) return
  logBusy.value = true
  try {
    const path = await api.openInstanceLog(editingId.value)
    Message.success(t('instanceEdit.logOpened', { path }))
  } catch (e) {
    Message.error(String(e))
  } finally {
    logBusy.value = false
  }
}

// --- Save ----------------------------------------------------------------------

const formValid = computed(
  () => name.value.trim().length > 0 && !!versionId.value && !!homeId.value && envValid.value,
)

async function onSave() {
  if (!formValid.value) return
  const envOverrides: Record<string, string> = {}
  for (const row of envRows.value) {
    if (row.key) envOverrides[row.key] = row.value
  }
  saving.value = true
  try {
    let resolvedHomeId = homeId.value!
    if (homeId.value === DEDICATED) {
      const home = await api.createHome(name.value.trim(), dedicatedPath.value)
      resolvedHomeId = home.id
      await store.refreshHomes()
    }
    const inst = store.instanceById(editingId.value!) as DshInstance
    await api.updateInstance({
      ...inst,
      name: name.value.trim(),
      version_id: versionId.value!,
      home_id: resolvedHomeId,
      env_overrides: envOverrides,
      default_profile: defaultProfile.value ?? null,
    })
    await store.refreshInstances()
    Message.success(t('instanceEdit.saved'))
    router.push({ name: 'instances' })
  } catch (e) {
    Message.error(String(e))
  } finally {
    saving.value = false
  }
}

function addEnvRow() {
  envRows.value.push({ key: '', value: '' })
}

function removeEnvRow(idx: number) {
  envRows.value.splice(idx, 1)
}

const homeLabel = computed(() => {
  if (!homeId.value) return '—'
  if (homeId.value === DEDICATED) return t('instanceEdit.dedicatedHome')
  return store.homeById(homeId.value)?.name ?? homeId.value
})
</script>

<template>
  <div class="edit-page">
    <section class="edit-content">
      <a-scrollbar type="track" outer-style="height: 100%" style="height: 100%; overflow-y: auto">
        <div class="edit-inner">
          <div class="dl-card context-bar">
            <img v-if="iconUrl" :src="iconUrl" class="context-icon" alt="" />
            <img v-else src="@/assets/launcher-icon.png" class="context-icon" alt="" />
            <div class="context-main">
              <div class="context-name">{{ name.trim() || t('instanceEdit.titleEdit') }}</div>
              <div class="context-meta">
                {{ t('instanceEdit.version') }}：{{ store.versionById(versionId ?? '')?.version ?? '—' }}
                · {{ t('instanceEdit.home') }}：{{ homeLabel }}
                <template v-if="defaultProfile"> · {{ t('instanceEdit.defaultProfile') }}：{{ defaultProfile }}</template>
              </div>
            </div>
            <a-tag v-if="editingId" size="small" color="arcoblue">{{ editingId.slice(0, 8) }}</a-tag>
          </div>

          <div class="dl-card edit-card">
            <a-form layout="vertical" class="edit-form" :model="{}">
              <a-form-item :label="t('instanceEdit.name')" required>
                <a-input v-model="name" :placeholder="t('instanceEdit.namePlaceholder')" style="max-width: 360px" />
              </a-form-item>

              <a-form-item v-if="editingId" :label="t('instanceEdit.icon')">
                <div class="icon-editor">
                  <img v-if="iconUrl" :src="iconUrl" class="icon-preview" alt="" />
                  <img v-else src="@/assets/launcher-icon.png" class="icon-preview" alt="" />
                  <div class="icon-actions">
                    <a-input
                      v-model="iconInput"
                      :placeholder="t('instanceEdit.iconUrlHint')"
                      allow-clear
                      style="max-width: 300px"
                    />
                    <a-space>
                      <a-button size="small" :loading="iconBusy" :disabled="!iconInput.trim()" @click="applyIconInput">
                        {{ t('instanceEdit.iconApply') }}
                      </a-button>
                      <a-button size="small" :loading="iconBusy" @click="pickIconFile">
                        {{ t('instanceEdit.iconPickFile') }}
                      </a-button>
                      <a-button v-if="iconUrl" size="small" status="danger" @click="clearIcon">
                        {{ t('instanceEdit.iconClear') }}
                      </a-button>
                    </a-space>
                    <p class="icon-hint">{{ t('instanceEdit.iconHint') }}</p>
                  </div>
                </div>
              </a-form-item>

              <a-form-item :label="t('instanceEdit.version')" required>
                <a-select v-model="versionId" style="max-width: 360px">
                  <a-option v-for="v in store.versions" :key="v.id" :value="v.id">{{ v.version }}</a-option>
                </a-select>
              </a-form-item>

              <a-form-item :label="t('instanceEdit.home')" required>
                <a-select v-model="homeId" style="max-width: 360px">
                  <a-option :value="DEDICATED">{{ t('instanceEdit.dedicatedHome') }}</a-option>
                  <a-option v-for="h in store.homes" :key="h.id" :value="h.id">
                    {{ h.name }}（{{ h.path }}）
                  </a-option>
                </a-select>
                <a-alert v-if="homeId === DEDICATED" type="info" class="dedicated-hint">
                  {{ t('instanceEdit.dedicatedHomeHint', { path: dedicatedPath }) }}
                </a-alert>
              </a-form-item>

              <a-form-item
                v-if="homeId && homeId !== DEDICATED"
                :label="t('instanceEdit.defaultProfile')"
              >
                <a-select
                  v-model="defaultProfile"
                  :placeholder="t('instanceEdit.defaultProfilePlaceholder')"
                  allow-clear
                  style="max-width: 360px"
                >
                  <a-option v-for="p in profiles" :key="p" :value="p">{{ p }}</a-option>
                </a-select>
                <p class="icon-hint">{{ t('instanceEdit.defaultProfileHint') }}</p>
              </a-form-item>

              <a-form-item :label="t('instanceEdit.env')">
                <p class="icon-hint">{{ t('instanceEdit.envDesc') }}</p>
                <div v-for="(row, idx) in envRows" :key="idx" class="env-row">
                  <a-input
                    v-model="row.key"
                    :placeholder="t('instanceEdit.envKey')"
                    :status="envKeyError(row) ? 'error' : undefined"
                    class="env-key"
                  />
                  <a-input v-model="row.value" :placeholder="t('instanceEdit.envValue')" class="env-value" />
                  <a-button status="danger" type="text" @click="removeEnvRow(idx)">
                    {{ t('instances.table.delete') }}
                  </a-button>
                  <div v-if="envKeyError(row)" class="env-error">{{ envKeyError(row) }}</div>
                </div>
                <a-empty v-if="envRows.length === 0" :description="t('instanceEdit.envAdd')" />
                <a-button size="small" class="env-add-btn" @click="addEnvRow">{{ t('instanceEdit.envAdd') }}</a-button>
              </a-form-item>

              <a-form-item v-if="editingId" :label="t('instanceEdit.port')">
                <a-space>
                  <a-input
                    v-model="portInput"
                    :placeholder="t('instanceEdit.portPlaceholder')"
                    allow-clear
                    style="width: 200px"
                    @press-enter="applyPort"
                  />
                  <a-button size="small" :loading="portBusy" @click="applyPort">
                    {{ t('instanceEdit.portApply') }}
                  </a-button>
                </a-space>
                <p class="icon-hint">{{ t('instanceEdit.portHint') }}</p>
              </a-form-item>

              <a-form-item v-if="editingId" :label="t('instanceEdit.files')">
                <a-space>
                  <a-button size="small" :loading="dirBusy" @click="onOpenDirectory">
                    {{ t('instanceEdit.openDirectory') }}
                  </a-button>
                  <a-button size="small" :loading="logBusy" @click="onViewLog">
                    {{ t('instanceEdit.viewLog') }}
                  </a-button>
                </a-space>
                <p class="icon-hint">{{ t('instanceEdit.filesHint') }}</p>
              </a-form-item>
            </a-form>

            <div class="footer-actions">
              <a-button type="primary" size="large" :disabled="!formValid" :loading="saving" @click="onSave">
                {{ t('instanceEdit.save') }}
              </a-button>
              <a-button size="large" @click="router.push({ name: 'instances' })">{{ t('instanceEdit.cancel') }}</a-button>
            </div>
          </div>
        </div>
      </a-scrollbar>
    </section>
  </div>
</template>

<style lang="scss" scoped>
.icon-editor {
  display: flex;
  gap: 16px;
  align-items: flex-start;
}

.icon-preview {
  width: 64px;
  height: 64px;
  border-radius: 12px;
  object-fit: cover;
  flex-shrink: 0;
  border: 1px solid var(--color-border-2);
}

.icon-actions {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.icon-hint {
  margin: 0;
  font-size: 12px;
  color: var(--color-text-3);
}

.edit-page {
  display: flex;
  height: calc(100vh - var(--dl-header-height));
}

.edit-content {
  flex: 1;
  min-width: 0;
  overflow: hidden;
}

.edit-inner {
  padding: 20px 24px 80px;
  max-width: 860px;
  margin: 0 auto;
}

.edit-card {
  width: 100%;
  box-sizing: border-box;

  & + & {
    margin-top: 16px;
  }
}

.context-bar {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 16px;
  padding: 14px 20px;
}

.context-icon {
  width: 40px;
  height: 40px;
  border-radius: 10px;
  object-fit: cover;
  flex-shrink: 0;
  border: 1px solid var(--color-border-2);
}

.context-main {
  flex: 1;
  min-width: 0;
}

.context-name {
  font-size: 15px;
  font-weight: 700;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.context-meta {
  margin-top: 2px;
  font-size: 12px;
  color: var(--color-text-3);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.edit-form {
  width: 100%;
}

.dedicated-hint {
  margin-top: 8px;
  max-width: 360px;
}

.env-row {
  display: flex;
  gap: 8px;
  align-items: center;
  flex-wrap: wrap;
  margin-bottom: 10px;
}

.env-key {
  width: 240px;
  font-family: monospace;
}

.env-value {
  flex: 1;
  min-width: 220px;
}

.env-error {
  width: 100%;
  color: rgb(var(--red-6));
  font-size: 12px;
}

.env-add-btn {
  margin-top: 4px;
}

.footer-actions {
  margin-top: 20px;
  display: flex;
  gap: 12px;
  justify-content: center;
  position: sticky;
  bottom: 0;
  padding: 12px 0 4px;
  background: linear-gradient(transparent, var(--color-bg-2) 32%);
  z-index: 5;
}
</style>
