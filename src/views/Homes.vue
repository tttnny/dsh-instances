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

// --- Plugins of the selected profile -----------------------------------------

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
  { title: t('instances.table.actions'), slotName: 'actions', width: 90, align: 'right' as const },
])

const pluginColumns = computed(() => [
  { title: 'ID', slotName: 'pid', width: 320 },
  { title: t('homes.pluginVersion'), slotName: 'pver', width: 130 },
  { title: t('homes.pluginStatus'), slotName: 'pstatus', width: 110 },
  { title: t('instances.table.actions'), slotName: 'pact', width: 80, align: 'right' as const },
])
</script>

<template>
  <div class="dl-page homes-page">
    <!-- Home Management Card -->
    <div class="dl-card home-mgmt-card">
      <div class="dl-card-title">
        <h3>{{ t('homes.homesTitle') }}</h3>
      </div>
      <p class="dl-card-desc">{{ t('homes.homesDesc') }}</p>

      <div class="home-add-row">
        <input
          v-model="newHomeName"
          type="text"
          class="apple-input-sm"
          :placeholder="t('homes.homeNamePlaceholder')"
          style="width: 170px"
        />
        <div class="path-input-group">
          <input
            v-model="newHomePath"
            type="text"
            class="apple-input-sm path-input"
            :placeholder="t('homes.homePathPlaceholder')"
          />
          <button class="mac-secondary-btn" @click="onPickDir">
            {{ t('homes.pickDir') }}
          </button>
        </div>
        <button
          class="mac-primary-btn"
          :disabled="!newHomeName.trim() || !newHomePath.trim()"
          @click="onAddHome"
        >
          {{ t('homes.addHome') }}
        </button>
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
        class="apple-styled-table"
        @selection-change="(keys: (string | number)[]) => (selectedHomeId = String(keys[0] ?? ''))"
      >
        <template #usedBy="{ record }">
          <span class="home-used tnum">{{ t('homes.homeUsedByCount', { count: homeUsedBy(record.id) }) }}</span>
        </template>
        <template #actions="{ record }">
          <a-popconfirm
            :content="t('homes.confirmDeleteHome', { name: record.name })"
            :disabled="homeUsedBy(record.id) > 0"
            @ok="onRemoveHome(record.id)"
          >
            <button
              class="mac-action-pill danger"
              :disabled="homeUsedBy(record.id) > 0"
            >
              {{ t('homes.deleteHome') }}
            </button>
          </a-popconfirm>
        </template>
        <template #empty>
          <a-empty :description="t('homes.homesEmpty')" />
        </template>
      </a-table>
    </div>

    <!-- Profiles & Plugins Section -->
    <div class="dl-card profile-mgmt-card">
      <div class="dl-card-title">
        <div class="title-with-pill">
          <h3>{{ t('homes.profilesTitle') }}</h3>
          <span v-if="selectedHome" class="home-tag-pill">{{ selectedHome.name }}</span>
        </div>
      </div>
      <p class="dl-card-desc">{{ t('homes.profilesDesc') }}</p>

      <template v-if="selectedHomeId">
        <div v-if="instancesOfHome(selectedHomeId).length > 0" class="used-by-pill-box">
          <span class="used-by-label">{{ t('homes.usedByInstances') }}：</span>
          <div class="used-by-list">
            <span v-for="inst in instancesOfHome(selectedHomeId)" :key="inst.id" class="used-by-item">
              <span class="inst-chip-name">{{ inst.name }}</span>
              <span class="inst-chip-desc">（{{ t('homes.defaultOf', { instance: inst.name }) }}：{{ inst.default_profile ?? t('homes.noDefault') }}）</span>
              <a-select
                :model-value="inst.default_profile ?? undefined"
                :placeholder="t('homes.noDefault')"
                allow-clear
                size="mini"
                style="width: 130px"
                @change="(v: unknown) => v && setDefaultProfile(inst.id, String(v))"
              >
                <a-option v-for="p in profiles" :key="p" :value="p">{{ p }}</a-option>
              </a-select>
            </span>
          </div>
        </div>
        <p v-else class="dl-card-desc">{{ t('homes.noInstances') }}</p>

        <div v-if="profilesLoading" class="dl-card-desc">{{ t('common.loading') }}</div>
        <a-empty v-else-if="profiles.length === 0" :description="t('homes.profilesEmpty')" />

        <!-- Profile Items Inset List -->
        <div class="profile-items-group">
          <div v-for="p in profiles" :key="p" class="profile-item-row">
            <template v-if="renamingProfile === p">
              <input v-model="renameValue" class="apple-input-sm" @press-enter="confirmRenameProfile" />
              <div class="inline-btn-group">
                <button class="mac-primary-btn" :disabled="busyProfile === p" @click="confirmRenameProfile">
                  {{ t('homes.profileRenameSave') }}
                </button>
                <button class="mac-secondary-btn" @click="renamingProfile = null">
                  {{ t('common.cancel') }}
                </button>
              </div>
            </template>

            <template v-else-if="copyingProfile === p">
              <input v-model="copyProfileName" class="apple-input-sm" @press-enter="confirmCopyProfile" />
              <div class="inline-btn-group">
                <button class="mac-primary-btn" :disabled="copyProfileBusy" @click="confirmCopyProfile">
                  {{ t('homes.profileCopySave') }}
                </button>
                <button class="mac-secondary-btn" @click="copyingProfile = null">
                  {{ t('common.cancel') }}
                </button>
              </div>
            </template>

            <template v-else>
              <div class="profile-left-col" @click="selectedPluginProfile = p">
                <div class="apple-radio-circle" :class="{ checked: selectedPluginProfile === p }">
                  <div class="inner-dot" />
                </div>
                <span class="profile-title">{{ p }}</span>
              </div>
              <div class="profile-item-actions">
                <button class="mac-micro-btn" @click="startRenameProfile(p)">{{ t('homes.profileRename') }}</button>
                <button class="mac-micro-btn" @click="startCopyProfile(p)">{{ t('homes.profileCopy') }}</button>
                <a-popconfirm
                  :content="t('homes.profileDeleteConfirm', { name: p })"
                  @ok="confirmDeleteProfile(p)"
                >
                  <button class="mac-micro-btn danger" :disabled="busyProfile === p">
                    {{ t('instances.table.delete') }}
                  </button>
                </a-popconfirm>
              </div>
            </template>
          </div>

          <div v-if="addingProfile" class="profile-item-row is-editing">
            <input
              v-model="newProfileName"
              :placeholder="t('homes.profileCreatePlaceholder')"
              class="apple-input-sm"
              @press-enter="onCreateProfile"
            />
            <div class="inline-btn-group">
              <button class="mac-primary-btn" :disabled="creatingProfile" @click="onCreateProfile">
                {{ t('homes.profileCreate') }}
              </button>
              <button class="mac-secondary-btn" @click="addingProfile = false">
                {{ t('common.cancel') }}
              </button>
            </div>
          </div>
        </div>

        <button v-if="!addingProfile" class="mac-secondary-btn" style="margin-top: 10px" @click="addingProfile = true">
          <svg viewBox="0 0 16 16" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2">
            <line x1="8" y1="3" x2="8" y2="13" />
            <line x1="3" y1="8" x2="13" y2="8" />
          </svg>
          <span>{{ t('homes.profileAdd') }}</span>
        </button>

        <!-- Plugins Section -->
        <div class="plugins-section">
          <div class="plugins-header">
            <h4>{{ t('homes.pluginsTitle') }}</h4>
            <span v-if="selectedPluginProfile" class="current-profile-pill">
              {{ selectedPluginProfile }}
            </span>
          </div>
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
              class="apple-styled-table"
              @selection-change="(keys: (string | number)[]) => (selectedPlugins = keys.map(String))"
            >
              <template #pid="{ record }">
                <span class="plugin-cell-id tnum">{{ record.id }}</span>
              </template>
              <template #pver="{ record }">
                <span v-if="record.version" class="tnum">{{ displayVersion(record.version) }}</span>
                <span v-else class="plugin-no-version">-</span>
              </template>
              <template #pstatus="{ record }">
                <a-switch
                  :model-value="record.enabled"
                  :disabled="pluginsBusy"
                  :checked-text="t('homes.pluginOn')"
                  :unchecked-text="t('homes.pluginOff')"
                  size="small"
                  @change="(v: string | number | boolean) => onTogglePlugin(record, v === true)"
                />
              </template>
              <template #pact="{ record }">
                <a-popconfirm
                  :content="t('homes.pluginUninstallConfirm', { name: record.id })"
                  @ok="onUninstallPlugin(record)"
                >
                  <button class="mac-action-pill danger" :disabled="pluginsBusy">
                    {{ t('instances.table.delete') }}
                  </button>
                </a-popconfirm>
              </template>
            </a-table>

            <div class="plugins-batch">
              <button
                class="mac-primary-btn"
                :disabled="selectedPlugins.length === 0 || pluginsBusy"
                @click="batchSetEnabled(true)"
              >
                {{ t('homes.pluginsBatchEnable', { count: selectedPlugins.length }) }}
              </button>
              <button
                class="mac-action-pill danger"
                :disabled="selectedPlugins.length === 0 || pluginsBusy"
                @click="batchSetEnabled(false)"
              >
                {{ t('homes.pluginsBatchDisable', { count: selectedPlugins.length }) }}
              </button>
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
  gap: 18px;
}

.home-add-row {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 16px;
  flex-wrap: wrap;

  .path-input-group {
    flex: 1;
    min-width: 260px;
    display: flex;
    align-items: center;
    gap: 6px;

    .path-input {
      flex: 1;
    }
  }
}

.apple-input-sm {
  height: 30px;
  padding: 0 10px;
  font-size: 13px;
  border-radius: 7px;
  border: 1px solid var(--apple-card-border);
  background: var(--apple-group-bg);
  color: var(--color-text-1);
  outline: none;
  transition: all 0.16s ease;

  &:focus {
    background: var(--apple-card-bg);
    border-color: rgb(var(--primary-6));
    box-shadow: 0 0 0 2px rgb(var(--primary-6) / 18%);
  }
}

.mac-primary-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 30px;
  padding: 0 12px;
  font-size: 12.5px;
  font-weight: 500;
  border-radius: 7px;
  border: none;
  background: rgb(var(--primary-6));
  color: #fff;
  cursor: pointer;
  transition: all 0.16s ease;

  &:hover:not(:disabled) {
    filter: brightness(1.06);
  }

  &:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  &:active:not(:disabled) {
    transform: scale(var(--apple-active-scale));
  }
}

.mac-secondary-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 30px;
  padding: 0 10px;
  font-size: 12.5px;
  font-weight: 500;
  border-radius: 7px;
  border: 1px solid var(--apple-card-border);
  background: var(--apple-card-bg);
  color: var(--color-text-2);
  cursor: pointer;
  transition: all 0.16s ease;

  &:hover {
    background: var(--apple-group-bg);
    color: var(--color-text-1);
  }

  &:active {
    transform: scale(var(--apple-active-scale));
  }
}

.mac-micro-btn {
  padding: 3px 8px;
  font-size: 12px;
  border-radius: 6px;
  border: 1px solid transparent;
  background: transparent;
  color: var(--color-text-2);
  cursor: pointer;
  transition: all 0.15s ease;

  &:hover:not(:disabled) {
    background: var(--apple-group-bg);
    color: var(--color-text-1);
    border-color: var(--apple-card-border);
  }

  &.danger {
    color: rgb(var(--red-6));

    &:hover:not(:disabled) {
      background: rgb(var(--red-6) / 12%);
    }
  }

  &:active:not(:disabled) {
    transform: scale(var(--apple-active-scale));
  }
}

.mac-action-pill {
  border: 1px solid var(--apple-card-border);
  background: var(--apple-card-bg);
  color: var(--color-text-2);
  border-radius: 6px;
  padding: 3px 9px;
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.15s ease;

  &:hover:not(:disabled) {
    background: var(--apple-group-bg);
    color: var(--color-text-1);
  }

  &.danger {
    color: rgb(var(--red-6));

    &:hover:not(:disabled) {
      background: rgb(var(--red-6) / 12%);
    }
  }

  &:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  &:active:not(:disabled) {
    transform: scale(var(--apple-active-scale));
  }
}

.title-with-pill {
  display: flex;
  align-items: center;
  gap: 10px;

  .home-tag-pill {
    padding: 2px 8px;
    font-size: 11.5px;
    font-weight: 600;
    border-radius: 6px;
    background: rgb(var(--primary-6) / 14%);
    color: rgb(var(--primary-6));
  }
}

.used-by-pill-box {
  background: var(--apple-group-bg);
  border-radius: 8px;
  padding: 10px 14px;
  margin-bottom: 14px;

  .used-by-label {
    font-size: 12.5px;
    font-weight: 600;
    color: var(--color-text-2);
  }

  .used-by-list {
    margin-top: 6px;
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .used-by-item {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: var(--apple-card-bg);
    border: 1px solid var(--apple-card-border);
    padding: 3px 8px;
    border-radius: 6px;
    font-size: 12px;

    .inst-chip-name {
      font-weight: 600;
      color: var(--color-text-1);
    }

    .inst-chip-desc {
      color: var(--color-text-3);
    }
  }
}

// Inset Group for Profiles
.profile-items-group {
  border: 1px solid var(--apple-card-border);
  border-radius: 9px;
  overflow: hidden;
  background: var(--apple-card-bg);
}

.profile-item-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 9px 14px;
  border-bottom: 1px solid var(--apple-separator);
  transition: background 0.15s ease;

  &:last-child {
    border-bottom: none;
  }

  &:hover {
    background: var(--apple-group-bg);
  }

  .profile-left-col {
    display: flex;
    align-items: center;
    gap: 10px;
    cursor: pointer;
    flex: 1;

    .apple-radio-circle {
      width: 14px;
      height: 14px;
      border-radius: 50%;
      border: 1.5px solid var(--color-text-4);
      display: flex;
      align-items: center;
      justify-content: center;
      transition: all 0.15s ease;

      .inner-dot {
        width: 6px;
        height: 6px;
        border-radius: 50%;
        background: transparent;
        transition: all 0.15s ease;
      }

      &.checked {
        border-color: rgb(var(--primary-6));

        .inner-dot {
          background: rgb(var(--primary-6));
        }
      }
    }

    .profile-title {
      font-size: 13px;
      font-weight: 500;
      color: var(--color-text-1);
    }
  }

  .profile-item-actions {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .inline-btn-group {
    display: flex;
    align-items: center;
    gap: 6px;
  }
}

// Plugins Section
.plugins-section {
  margin-top: 24px;
  padding-top: 18px;
  border-top: 1px solid var(--apple-separator);

  .plugins-header {
    display: flex;
    align-items: center;
    gap: 10px;

    h4 {
      margin: 0;
      font-size: 14px;
      font-weight: 600;
      color: var(--color-text-1);
    }

    .current-profile-pill {
      font-size: 11.5px;
      font-weight: 600;
      padding: 1px 7px;
      border-radius: 6px;
      background: rgb(var(--green-6) / 14%);
      color: rgb(var(--green-6));
    }
  }
}

.plugin-cell-id {
  font-family: 'SF Mono', Menlo, Monaco, Consolas, monospace;
  font-size: 12px;
}

.plugins-batch {
  margin-top: 12px;
  display: flex;
  gap: 8px;
}
</style>
