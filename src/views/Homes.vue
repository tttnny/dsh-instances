<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Message } from '@arco-design/web-vue'
import { api } from '@/api'
import { useLauncherStore } from '@/stores/launcher'
import type { InstalledPlugin } from '@/api/types'

const { t } = useI18n()
const store = useLauncherStore()

// --- HOME list ---------------------------------------------------------------

const newHomeName = ref('')
const newHomePath = ref('')

function homeUsedBy(id: string): number {
  return store.instances.filter((i) => i.home_id === id).length
}

function instancesOfHome(homeId: string) {
  return store.instances.filter((i) => i.home_id === homeId)
}

async function onPickDir() {
  if (api.isTauri) {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog')
      const dir = await open({ directory: true, multiple: false })
      if (typeof dir === 'string') newHomePath.value = dir
    } catch (e) {
      Message.error(String(e))
    }
  } else {
    Message.info(t('homes.browserPickHint'))
  }
}

async function onAddHome() {
  try {
    await api.createHome(newHomeName.value, newHomePath.value)
    newHomeName.value = ''
    newHomePath.value = ''
    await store.refreshHomes()
    Message.success(t('settings.saved'))
  } catch (e) {
    Message.error(String(e))
  }
}

async function onRemoveHome(id: string) {
  try {
    await api.removeHome(id)
    await store.refreshHomes()
    if (selectedHomeId.value === id) selectedHomeId.value = store.homes[0]?.id
  } catch (e) {
    Message.error(String(e))
  }
}

// --- Profiles of the selected HOME -------------------------------------------

const selectedHomeId = ref<string | undefined>(store.homes[0]?.id)
const profiles = ref<string[]>([])
const profilesLoading = ref(false)
const newProfileName = ref('')
const addingProfile = ref(false)
const creatingProfile = ref(false)
const renamingProfile = ref<string | null>(null)
const renameValue = ref('')
const copyingProfile = ref<string | null>(null)
const copyProfileName = ref('')
const copyProfileBusy = ref(false)
const busyProfile = ref<string | null>(null)

const selectedHome = computed(() => store.homeById(selectedHomeId.value ?? ''))

// Plugin selection lives next to the HOME selection: the watcher below
// resets it, so it must be declared before the watcher runs.
const selectedPluginProfile = ref('')

async function loadProfiles() {
  profiles.value = []
  if (!selectedHomeId.value) return
  profilesLoading.value = true
  try {
    profiles.value = await api.listProfiles(selectedHomeId.value)
  } catch (e) {
    Message.error(String(e))
  } finally {
    profilesLoading.value = false
  }
}

watch(selectedHomeId, () => {
  void loadProfiles()
  selectedPluginProfile.value = ''
}, { immediate: true })

async function onCreateProfile() {
  const name = newProfileName.value.trim()
  if (!selectedHomeId.value || !name) return
  creatingProfile.value = true
  try {
    await api.createProfile(selectedHomeId.value, name)
    profiles.value = await api.listProfiles(selectedHomeId.value)
    newProfileName.value = ''
    addingProfile.value = false
    Message.success(t('homes.profileCreated', { name }))
  } catch (e) {
    Message.error(String(e))
  } finally {
    creatingProfile.value = false
  }
}

function startRenameProfile(name: string) {
  renamingProfile.value = name
  renameValue.value = name
}

async function confirmRenameProfile() {
  if (!selectedHomeId.value || !renamingProfile.value) return
  const oldName = renamingProfile.value
  const newName = renameValue.value.trim()
  if (!newName || newName === oldName) {
    renamingProfile.value = null
    return
  }
  busyProfile.value = oldName
  try {
    await api.renameProfile(selectedHomeId.value, oldName, newName)
    profiles.value = await api.listProfiles(selectedHomeId.value)
    renamingProfile.value = null
    await store.refreshInstances()
    Message.success(t('homes.profileRenamed', { old: oldName, name: newName }))
  } catch (e) {
    Message.error(String(e))
  } finally {
    busyProfile.value = null
  }
}

function startCopyProfile(name: string) {
  copyingProfile.value = name
  copyProfileName.value = `${name}-copy`
}

async function confirmCopyProfile() {
  if (!selectedHomeId.value || !copyingProfile.value) return
  const source = copyingProfile.value
  const newName = copyProfileName.value.trim()
  if (!newName) return
  copyProfileBusy.value = true
  try {
    await api.copyProfile(selectedHomeId.value, source, newName)
    profiles.value = await api.listProfiles(selectedHomeId.value)
    copyingProfile.value = null
    Message.success(t('homes.profileCopied', { source, name: newName }))
  } catch (e) {
    Message.error(String(e))
  } finally {
    copyProfileBusy.value = false
  }
}

async function confirmDeleteProfile(name: string) {
  if (!selectedHomeId.value) return
  busyProfile.value = name
  try {
    await api.deleteProfile(selectedHomeId.value, name)
    profiles.value = await api.listProfiles(selectedHomeId.value)
    await store.refreshInstances()
    Message.success(t('homes.profileDeleted', { name }))
  } catch (e) {
    Message.error(String(e))
  } finally {
    busyProfile.value = null
  }
}

async function setDefaultProfile(instanceId: string, profile: string) {
  const inst = store.instanceById(instanceId)
  if (!inst) return
  try {
    await api.updateInstance({ ...inst, default_profile: profile })
    await store.refreshInstances()
    Message.success(t('homes.profileSetDefault', { name: profile }))
  } catch (e) {
    Message.error(String(e))
  }
}

// --- Plugins of the selected profile ------------------------------------------

const installedPlugins = ref<InstalledPlugin[]>([])
const pluginsLoading = ref(false)
const selectedPlugins = ref<string[]>([])
const pluginsBusy = ref(false)

const visiblePlugins = computed(() =>
  installedPlugins.value.filter((p) => !p.id.startsWith('@deepseek-ai/')),
)

function displayVersion(v: string | undefined): string {
  if (v && /^[0-9a-f]{40}$/i.test(v)) return v.slice(0, 7)
  return v ?? ''
}

watch([selectedPluginProfile, selectedHomeId], async () => {
  await loadPlugins()
})

async function loadPlugins() {
  installedPlugins.value = []
  selectedPlugins.value = []
  if (!selectedHomeId.value || !selectedPluginProfile.value) return
  pluginsLoading.value = true
  try {
    installedPlugins.value = await api.listInstalledPlugins(
      selectedHomeId.value,
      selectedPluginProfile.value,
    )
  } catch (e) {
    Message.error(String(e))
  } finally {
    pluginsLoading.value = false
  }
}

async function onTogglePlugin(p: InstalledPlugin, enabled: boolean) {
  if (!selectedHomeId.value || !selectedPluginProfile.value) return
  pluginsBusy.value = true
  try {
    await api.setPluginsEnabled({
      home_id: selectedHomeId.value,
      profile: selectedPluginProfile.value,
      pluginIds: [p.id],
      enabled,
    })
    p.enabled = enabled
    Message.success(
      enabled
        ? t('homes.pluginEnabled', { name: p.id })
        : t('homes.pluginDisabled', { name: p.id }),
    )
  } catch (e) {
    Message.error(String(e))
    await loadPlugins()
  } finally {
    pluginsBusy.value = false
  }
}

async function onUninstallPlugin(p: InstalledPlugin) {
  if (!selectedHomeId.value || !selectedPluginProfile.value) return
  pluginsBusy.value = true
  try {
    await api.uninstallPlugin({
      home_id: selectedHomeId.value,
      profile: selectedPluginProfile.value,
      pluginId: p.id,
    })
    Message.success(t('homes.pluginUninstalled', { name: p.id }))
    Message.info(t('homes.pluginRestartHint'))
    await loadPlugins()
  } catch (e) {
    Message.error(String(e))
  } finally {
    pluginsBusy.value = false
  }
}

async function batchSetEnabled(enabled: boolean) {
  if (!selectedHomeId.value || !selectedPluginProfile.value || selectedPlugins.value.length === 0) return
  pluginsBusy.value = true
  const ids = [...selectedPlugins.value]
  try {
    await api.setPluginsEnabled({
      home_id: selectedHomeId.value,
      profile: selectedPluginProfile.value,
      pluginIds: ids,
      enabled,
    })
    for (const p of installedPlugins.value) {
      if (ids.includes(p.id)) p.enabled = enabled
    }
    selectedPlugins.value = []
    Message.success(
      enabled
        ? t('homes.pluginsBatchEnabled', { count: ids.length })
        : t('homes.pluginsBatchDisabled', { count: ids.length }),
    )
  } catch (e) {
    Message.error(String(e))
    await loadPlugins()
  } finally {
    pluginsBusy.value = false
  }
}

const homeColumns = computed(() => [
  { title: t('homes.homeName'), dataIndex: 'name', width: 170 },
  { title: t('homes.homePath'), dataIndex: 'path', ellipsis: true, tooltip: true },
  { title: t('homes.homeUsedBy'), slotName: 'usedBy', width: 130 },
  { title: t('instances.table.actions'), slotName: 'actions', width: 90, align: 'center' as const },
])

const pluginColumns = computed(() => [
  { title: 'ID', slotName: 'pid', width: 320 },
  { title: t('homes.pluginVersion'), slotName: 'pver', width: 140 },
  { title: t('homes.pluginStatus'), slotName: 'pstatus', width: 120 },
  { title: t('instances.table.actions'), slotName: 'pact', width: 90 },
])
</script>

<template>
  <div class="dl-page homes-page">
    <div class="dl-card">
      <div class="dl-card-title">
        <h3>{{ t('homes.homesTitle') }}</h3>
      </div>
      <p class="dl-card-desc">{{ t('homes.homesDesc') }}</p>

      <div class="home-add-row">
        <a-input v-model="newHomeName" :placeholder="t('homes.homeNamePlaceholder')" style="width: 200px" />
        <a-input v-model="newHomePath" :placeholder="t('homes.homePathPlaceholder')" class="home-path-input" />
        <a-button @click="onPickDir">{{ t('homes.pickDir') }}</a-button>
        <a-button type="primary" :disabled="!newHomeName.trim() || !newHomePath.trim()" @click="onAddHome">
          {{ t('homes.addHome') }}
        </a-button>
      </div>

      <a-table
        :columns="homeColumns"
        :data="store.homes"
        :pagination="false"
        row-key="id"
        :row-selection="{
          type: 'radio',
          showCheckedAll: false,
          selectedRowKeys: selectedHomeId ? [selectedHomeId] : [],
        }"
        @selection-change="(keys: (string | number)[]) => (selectedHomeId = String(keys[0] ?? ''))"
      >
        <template #usedBy="{ record }">
          <span class="home-used">{{ t('homes.homeUsedByCount', { count: homeUsedBy(record.id) }) }}</span>
        </template>
        <template #actions="{ record }">
          <a-popconfirm
            :content="t('homes.confirmDeleteHome', { name: record.name })"
            :disabled="homeUsedBy(record.id) > 0"
            @ok="onRemoveHome(record.id)"
          >
            <a-button size="small" status="danger" :disabled="homeUsedBy(record.id) > 0">
              {{ t('homes.deleteHome') }}
            </a-button>
          </a-popconfirm>
        </template>
        <template #empty>
          <a-empty :description="t('homes.homesEmpty')" />
        </template>
      </a-table>
    </div>

    <div class="dl-card">
      <div class="dl-card-title">
        <h3>{{ t('homes.profilesTitle') }}</h3>
        <span v-if="selectedHome" class="profiles-home">{{ selectedHome.name }}</span>
      </div>
      <p class="dl-card-desc">{{ t('homes.profilesDesc') }}</p>

      <template v-if="selectedHomeId">
        <div v-if="instancesOfHome(selectedHomeId).length > 0" class="used-by">
          <span class="used-by-label">{{ t('homes.usedByInstances') }}：</span>
          <span v-for="inst in instancesOfHome(selectedHomeId)" :key="inst.id" class="used-by-item">
            {{ inst.name }}（{{ t('homes.defaultOf', { instance: inst.name }) }}：{{ inst.default_profile ?? t('homes.noDefault') }}）
            <a-select
              :model-value="inst.default_profile ?? undefined"
              :placeholder="t('homes.noDefault')"
              allow-clear
              size="mini"
              style="width: 140px"
              @change="(v: unknown) => v && setDefaultProfile(inst.id, String(v))"
            >
              <a-option v-for="p in profiles" :key="p" :value="p">{{ p }}</a-option>
            </a-select>
          </span>
        </div>
        <p v-else class="dl-card-desc">{{ t('homes.noInstances') }}</p>

        <div v-if="profilesLoading">{{ t('common.loading') }}</div>
        <a-empty v-else-if="profiles.length === 0" :description="t('homes.profilesEmpty')" />

        <div v-for="p in profiles" :key="p" class="profile-item">
          <template v-if="renamingProfile === p">
            <a-input v-model="renameValue" class="profile-item-name" @press-enter="confirmRenameProfile" />
            <a-button size="small" type="primary" :loading="busyProfile === p" @click="confirmRenameProfile">
              {{ t('homes.profileRenameSave') }}
            </a-button>
            <a-button size="small" @click="renamingProfile = null">{{ t('common.cancel') }}</a-button>
          </template>
          <template v-else-if="copyingProfile === p">
            <a-input v-model="copyProfileName" class="profile-item-name" @press-enter="confirmCopyProfile" />
            <a-button size="small" type="primary" :loading="copyProfileBusy" @click="confirmCopyProfile">
              {{ t('homes.profileCopySave') }}
            </a-button>
            <a-button size="small" @click="copyingProfile = null">{{ t('common.cancel') }}</a-button>
          </template>
          <template v-else>
            <a-radio
              :model-value="selectedPluginProfile === p"
              @change="selectedPluginProfile = p"
            >
              <span class="profile-item-name">{{ p }}</span>
            </a-radio>
            <span class="profile-item-actions">
              <a-button size="small" @click="startRenameProfile(p)">{{ t('homes.profileRename') }}</a-button>
              <a-button size="small" @click="startCopyProfile(p)">{{ t('homes.profileCopy') }}</a-button>
              <a-popconfirm
                :content="t('homes.profileDeleteConfirm', { name: p })"
                @ok="confirmDeleteProfile(p)"
              >
                <a-button size="small" status="danger" :loading="busyProfile === p">
                  {{ t('instances.table.delete') }}
                </a-button>
              </a-popconfirm>
            </span>
          </template>
        </div>

        <div v-if="addingProfile" class="profile-item">
          <a-input
            v-model="newProfileName"
            :placeholder="t('homes.profileCreatePlaceholder')"
            class="profile-item-name"
            @press-enter="onCreateProfile"
          />
          <a-button size="small" type="primary" :loading="creatingProfile" @click="onCreateProfile">
            {{ t('homes.profileCreate') }}
          </a-button>
          <a-button size="small" @click="addingProfile = false">{{ t('common.cancel') }}</a-button>
        </div>
        <a-button v-if="!addingProfile" size="small" @click="addingProfile = true">
          {{ t('homes.profileAdd') }}
        </a-button>

        <div class="plugins-section">
          <h4>{{ t('homes.pluginsTitle') }}</h4>
          <p class="dl-card-desc">{{ t('homes.pluginsDesc') }}</p>
          <template v-if="selectedPluginProfile">
            <a-table
              :columns="pluginColumns"
              :data="visiblePlugins"
              :loading="pluginsLoading"
              :pagination="false"
              row-key="id"
              :row-selection="{
                type: 'checkbox',
                showCheckedAll: true,
                onlyCurrent: true,
                selectedRowKeys: selectedPlugins,
              }"
              size="small"
              @selection-change="(keys: (string | number)[]) => (selectedPlugins = keys.map(String))"
            >
              <template #pid="{ record }">
                <span class="plugin-cell-id">{{ record.id }}</span>
              </template>
              <template #pver="{ record }">
                <span v-if="record.version">{{ displayVersion(record.version) }}</span>
                <span v-else class="plugin-no-version">-</span>
              </template>
              <template #pstatus="{ record }">
                <a-switch
                  :model-value="record.enabled"
                  :disabled="pluginsBusy"
                  :checked-text="t('homes.pluginOn')"
                  :unchecked-text="t('homes.pluginOff')"
                  @change="(v: string | number | boolean) => onTogglePlugin(record, v === true)"
                />
              </template>
              <template #pact="{ record }">
                <a-popconfirm
                  :content="t('homes.pluginUninstallConfirm', { name: record.id })"
                  @ok="onUninstallPlugin(record)"
                >
                  <a-button size="small" status="danger" :disabled="pluginsBusy">
                    {{ t('instances.table.delete') }}
                  </a-button>
                </a-popconfirm>
              </template>
            </a-table>
            <div class="plugins-batch">
              <a-button
                size="small"
                type="primary"
                :disabled="selectedPlugins.length === 0 || pluginsBusy"
                @click="batchSetEnabled(true)"
              >
                {{ t('homes.pluginsBatchEnable', { count: selectedPlugins.length }) }}
              </a-button>
              <a-button
                size="small"
                status="danger"
                :disabled="selectedPlugins.length === 0 || pluginsBusy"
                @click="batchSetEnabled(false)"
              >
                {{ t('homes.pluginsBatchDisable', { count: selectedPlugins.length }) }}
              </a-button>
            </div>
            <a-empty
              v-if="!pluginsLoading && visiblePlugins.length === 0"
              :description="t('homes.pluginsEmpty')"
            />
          </template>
          <a-empty v-else :description="t('homes.pluginsPickProfile')" />
        </div>
      </template>
      <a-empty v-else :description="t('homes.noHomeSelected')" />
    </div>
  </div>
</template>

<style lang="scss" scoped>
.homes-page {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.home-add-row {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-bottom: 16px;
}

.home-path-input {
  flex: 1;
}

.home-used {
  font-size: 12px;
  color: var(--color-text-3);
  white-space: nowrap;
}

.profiles-home {
  font-size: 12px;
  color: var(--color-text-3);
}

.used-by {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-bottom: 12px;
  font-size: 13px;
}

.used-by-label {
  color: var(--color-text-3);
}

.used-by-item {
  display: flex;
  align-items: center;
  gap: 8px;
}

.profile-item {
  display: flex;
  gap: 8px;
  align-items: center;
  padding: 10px 12px;
  border: 1px solid var(--color-border-2);
  border-radius: 6px;
  margin-bottom: 8px;
  background: var(--color-fill-1);
}

.profile-item-name {
  flex: 1;
  min-width: 0;
  font-size: 14px;
}

.profile-item-actions {
  display: inline-flex;
  gap: 8px;
  align-items: center;
  flex-shrink: 0;
}

.plugins-section {
  margin-top: 20px;
  border-top: 1px solid var(--color-border-1);
  padding-top: 16px;

  h4 {
    margin: 0 0 4px;
    font-size: 15px;
  }
}

.plugin-cell-id {
  font-family: monospace;
  font-size: 13px;
}

.plugin-no-version {
  color: var(--color-text-4);
}

.plugins-batch {
  display: flex;
  gap: 8px;
  margin: 8px 0;
}
</style>
