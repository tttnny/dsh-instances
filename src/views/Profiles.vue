<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { Message } from '@arco-design/web-vue'
import { api } from '@/api'
import { useLauncherStore } from '@/stores/launcher'

const { t } = useI18n()
const route = useRoute()
const router = useRouter()
const store = useLauncherStore()

// --- Selected HOME -----------------------------------------------------------

const selectedHomeId = ref<string | undefined>(undefined)

const selectedHome = computed(() => store.homeById(selectedHomeId.value ?? ''))

function instancesOfHome(homeId: string) {
  return store.instances.filter((i) => i.home_id === homeId)
}

// Watch route.query or store.homes to initialize / sync selectedHomeId
watch(
  [() => route.query.homeId, () => store.homes],
  ([qHomeId]) => {
    const qStr = typeof qHomeId === 'string' ? qHomeId : undefined
    if (qStr && store.homes.some((h) => h.id === qStr)) {
      selectedHomeId.value = qStr
    } else if (!selectedHomeId.value || !store.homes.some((h) => h.id === selectedHomeId.value)) {
      selectedHomeId.value = store.homes[0]?.id
    }
  },
  { immediate: true },
)

function onSelectHome(id: unknown) {
  const homeId = String(id ?? '')
  selectedHomeId.value = homeId
  void router.replace({ query: { ...route.query, homeId } })
}

// --- Profiles of the selected HOME -------------------------------------------

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

watch(
  selectedHomeId,
  () => {
    void loadProfiles()
  },
  { immediate: true },
)

async function onCreateProfile() {
  const name = newProfileName.value.trim()
  if (!selectedHomeId.value || !name) return
  creatingProfile.value = true
  try {
    await api.createProfile(selectedHomeId.value, name)
    newProfileName.value = ''
    addingProfile.value = false
    await loadProfiles()
    Message.success(t('profiles.profileCreated', { name }))
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
  const old = renamingProfile.value
  const name = renameValue.value.trim()
  if (!selectedHomeId.value || !old || !name || old === name) {
    renamingProfile.value = null
    return
  }
  busyProfile.value = old
  try {
    await api.renameProfile(selectedHomeId.value, old, name)
    await store.refreshInstances()
    await loadProfiles()
    renamingProfile.value = null
    Message.success(t('profiles.profileRenamed', { old, name }))
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
  const source = copyingProfile.value
  const name = copyProfileName.value.trim()
  if (!selectedHomeId.value || !source || !name) {
    copyingProfile.value = null
    return
  }
  copyProfileBusy.value = true
  try {
    await api.copyProfile(selectedHomeId.value, source, name)
    await loadProfiles()
    copyingProfile.value = null
    Message.success(t('profiles.profileCopied', { source, name }))
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
    await store.refreshInstances()
    await loadProfiles()
    Message.success(t('profiles.profileDeleted', { name }))
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
    Message.success(t('profiles.profileSetDefault', { name: profile }))
  } catch (e) {
    Message.error(String(e))
  }
}

function onManagePlugins(profile: string) {
  if (!selectedHomeId.value) return
  void router.push({
    path: '/plugins',
    query: {
      homeId: selectedHomeId.value,
      profile,
    },
  })
}
</script>

<template>
  <div class="dl-page profiles-page">
    <!-- Top Selector Bar -->
    <div class="dl-card home-selector-bar">
      <div class="selector-row">
        <label class="selector-label">{{ t('profiles.selectHome') }}：</label>
        <a-select
          :model-value="selectedHomeId"
          :placeholder="t('profiles.selectHome')"
          class="home-select"
          @change="onSelectHome"
        >
          <a-option v-for="h in store.homes" :key="h.id" :value="h.id">
            {{ h.name }} ({{ h.path }})
          </a-option>
        </a-select>
      </div>
    </div>

    <!-- Profiles Management Card -->
    <div v-if="selectedHomeId" class="dl-card profile-mgmt-card">
      <div class="dl-card-title">
        <div class="title-with-pill">
          <h3>{{ t('profiles.profilesTitle') }}</h3>
          <span v-if="selectedHome" class="home-tag-pill">{{ selectedHome.name }}</span>
        </div>
      </div>
      <p class="dl-card-desc">{{ t('profiles.profilesDesc') }}</p>

      <!-- Instances referencing this home -->
      <div v-if="instancesOfHome(selectedHomeId).length > 0" class="used-by-pill-box">
        <span class="used-by-label">{{ t('profiles.usedByInstances') }}：</span>
        <div class="used-by-list">
          <span v-for="inst in instancesOfHome(selectedHomeId)" :key="inst.id" class="used-by-item">
            <span class="inst-chip-name">{{ inst.name }}</span>
            <span class="inst-chip-desc">（{{ t('profiles.defaultOf', { instance: inst.name }) }}：{{ inst.default_profile ?? t('profiles.noDefault') }}）</span>
            <a-select
              :model-value="inst.default_profile ?? undefined"
              :placeholder="t('profiles.noDefault')"
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
      <p v-else class="dl-card-desc">{{ t('profiles.noInstances') }}</p>

      <div v-if="profilesLoading" class="dl-card-desc">{{ t('common.loading') }}</div>
      <a-empty v-else-if="profiles.length === 0" :description="t('profiles.profilesEmpty')" />

      <!-- Profile Items List -->
      <div class="profile-items-group">
        <div v-for="p in profiles" :key="p" class="profile-item-row">
          <template v-if="renamingProfile === p">
            <input v-model="renameValue" class="apple-input-sm" @press-enter="confirmRenameProfile" />
            <div class="inline-btn-group">
              <button class="mac-primary-btn" :disabled="busyProfile === p" @click="confirmRenameProfile">
                {{ t('profiles.profileRenameSave') }}
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
                {{ t('profiles.profileCopySave') }}
              </button>
              <button class="mac-secondary-btn" @click="copyingProfile = null">
                {{ t('common.cancel') }}
              </button>
            </div>
          </template>

          <template v-else>
            <div class="profile-left-col">
              <span class="profile-title">{{ p }}</span>
            </div>
            <div class="profile-item-actions">
              <button class="mac-action-pill highlight" @click="onManagePlugins(p)">
                {{ t('profiles.managePlugins') }}
              </button>
              <button class="mac-micro-btn" @click="startRenameProfile(p)">{{ t('profiles.profileRename') }}</button>
              <button class="mac-micro-btn" @click="startCopyProfile(p)">{{ t('profiles.profileCopy') }}</button>
              <a-popconfirm
                :content="t('profiles.profileDeleteConfirm', { name: p })"
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
            :placeholder="t('profiles.profileCreatePlaceholder')"
            class="apple-input-sm"
            @press-enter="onCreateProfile"
          />
          <div class="inline-btn-group">
            <button class="mac-primary-btn" :disabled="creatingProfile" @click="onCreateProfile">
              {{ t('profiles.profileCreate') }}
            </button>
            <button class="mac-secondary-btn" @click="addingProfile = false">
              {{ t('common.cancel') }}
            </button>
          </div>
        </div>
      </div>

      <button v-if="!addingProfile" class="mac-secondary-btn" style="margin-top: 12px" @click="addingProfile = true">
        <svg viewBox="0 0 16 16" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2">
          <line x1="8" y1="3" x2="8" y2="13" />
          <line x1="3" y1="8" x2="13" y2="8" />
        </svg>
        <span>{{ t('profiles.profileAdd') }}</span>
      </button>
    </div>

    <!-- Empty Home state -->
    <div v-else class="dl-card">
      <a-empty :description="t('profiles.noHomeSelected')" />
    </div>
  </div>
</template>

<style lang="scss" scoped>
.profiles-page {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.home-selector-bar {
  padding: 14px 18px;

  .selector-row {
    display: flex;
    align-items: center;
    gap: 12px;

    .selector-label {
      font-size: 13px;
      font-weight: 500;
      color: var(--color-text-2);
      white-space: nowrap;
    }

    .home-select {
      max-width: 420px;
    }
  }
}

.title-with-pill {
  display: flex;
  align-items: center;
  gap: 10px;

  h3 {
    margin: 0;
  }
}

.home-tag-pill {
  display: inline-flex;
  align-items: center;
  font-size: 12px;
  font-weight: 500;
  padding: 2px 8px;
  border-radius: 6px;
  background: rgb(var(--primary-6) / 10%);
  color: rgb(var(--primary-6));
  border: 1px solid rgb(var(--primary-6) / 20%);
}

.used-by-pill-box {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-bottom: 14px;
  padding: 10px 12px;
  border-radius: 8px;
  background: var(--apple-group-bg);
  border: 1px solid var(--apple-card-border);

  .used-by-label {
    font-size: 12px;
    font-weight: 500;
    color: var(--color-text-3);
  }

  .used-by-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .used-by-item {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-size: 12.5px;

    .inst-chip-name {
      font-weight: 500;
      color: var(--color-text-1);
    }

    .inst-chip-desc {
      color: var(--color-text-3);
      font-size: 12px;
    }
  }
}

.profile-items-group {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-top: 12px;
}

.profile-item-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 14px;
  border-radius: 8px;
  background: var(--apple-group-bg);
  border: 1px solid var(--apple-card-border);
  gap: 12px;

  &.is-editing {
    background: var(--apple-card-bg);
  }

  .profile-left-col {
    display: flex;
    align-items: center;
    gap: 10px;

    .profile-title {
      font-size: 13px;
      font-weight: 500;
      color: var(--color-text-1);
    }
  }

  .profile-item-actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .inline-btn-group {
    display: flex;
    align-items: center;
    gap: 8px;
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

  &.highlight {
    color: rgb(var(--primary-6));
    border-color: rgb(var(--primary-6) / 30%);
    background: rgb(var(--primary-6) / 8%);

    &:hover:not(:disabled) {
      background: rgb(var(--primary-6) / 16%);
    }
  }

  &:active:not(:disabled) {
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
</style>
