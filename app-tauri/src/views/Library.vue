<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from "vue";
import {
  api,
  onEvent,
  fmtDuration,
  type Recording,
} from "../api";
import DetailsDialog from "../components/DetailsDialog.vue";
import HotkeyDialog from "../components/HotkeyDialog.vue";
import StepEditor from "../components/StepEditor.vue";
import { filterRecordings } from "../lib/library";

const props = defineProps<{ recording: boolean; playing: boolean }>();

const recordings = ref<Recording[]>([]);
const search = ref("");
const filteredRecordings = computed(() => filterRecordings(recordings.value, search.value));
const selectedId = ref<string | null>(null);
const speed = ref(1);
const repeat = ref(1);
const waitSeconds = ref(3);
const liveCount = ref(0);
const countdown = ref<number | null>(null);
const playLoop = ref<{ done: number; total: number } | null>(null);
const details = ref<Recording | null>(null);
const detailsRef = ref<{ setCurrentMs: (ms: number | null) => void } | null>(null);
const playProgress = ref<{ index: number; total: number } | null>(null);
const busy = ref(false);
const screenText = ref("—");
const unlistens: Array<() => void> = [];

const selected = computed(() =>
  recordings.value.find((r) => r.id === selectedId.value) ?? null,
);
const lastRec = computed(() => recordings.value[0] ?? null);

const SPEED_OPTIONS = [
  { value: 0, label: "立即执行" },
  { value: 0.5, label: "0.5×" },
  { value: 1, label: "1×（原速）" },
  { value: 2, label: "2×" },
  { value: 5, label: "5×" },
];

async function load() {
  recordings.value = await api.listRecordings();
  if (!selected.value && recordings.value.length) {
    selectedId.value = recordings.value[0].id;
  }
}

async function loadSettings() {
  const s = await api.getSettings();
  waitSeconds.value = s.wait_seconds;
}

async function toggleRecord() {
  if (busy.value) return;
  busy.value = true;
  try {
    if (props.recording) {
      await api.endRecording();
    } else {
      await api.beginRecording();
    }
  } catch (e) {
    alert(String(e));
  } finally {
    busy.value = false;
  }
}

async function play() {
  if (!selected.value || props.playing || props.recording || busy.value) return;
  busy.value = true;
  try {
    await api.startPlayback(selected.value.id, speed.value, repeat.value, waitSeconds.value);
  } catch (e) {
    alert(String(e));
  } finally {
    busy.value = false;
  }
}

async function stop() {
  await api.abortPlayback();
}

async function remove(rec: Recording) {
  if (!confirm(`删除「${rec.name}」？`)) return;
  await api.deleteRecording(rec.id);
  if (selectedId.value === rec.id) selectedId.value = null;
  await load();
}

async function rename(rec: Recording) {
  const name = prompt("脚本名称", rec.name);
  if (!name || !name.trim()) return;
  await api.renameRecording(rec.id, name.trim());
  await load();
}

async function duplicate(rec: Recording) {
  if (busy.value) return;
  busy.value = true;
  try {
    const copy = await api.duplicateRecording(rec.id);
    search.value = "";
    await load();
    selectedId.value = copy.id;
  } catch (e) {
    alert(String(e));
  } finally {
    busy.value = false;
  }
}

const hotkeyTarget = ref<Recording | null>(null);
const editTarget = ref<Recording | null>(null);

async function saveHotkey(hotkey: string | null) {
  const rec = hotkeyTarget.value;
  if (!rec) return;
  hotkeyTarget.value = null;
  try {
    await api.setHotkey(rec.id, hotkey);
    await load();
  } catch (e) {
    alert(String(e));
  }
}

const HK_PRETTY: Record<string, string> = {
  Control: "Ctrl",
  Alt: "Alt",
  Shift: "Shift",
  Meta: "Win",
};
function prettyHk(spec: string | null | undefined): string {
  if (!spec) return "";
  return spec
    .split("+")
    .map((p) => HK_PRETTY[p] ?? p)
    .join("+");
}

function exportJson(rec: Recording) {
  const path = prompt(
    "导出到文件（绝对路径，例如 C:\\Users\\me\\Desktop\\macro.json）",
    `winmacro-${rec.id}.json`,
  );
  if (!path) return;
  api.exportRecording(rec.id, path)
    .then(() => alert("已导出"))
    .catch((e) => alert(String(e)));
}

function importJson() {
  const path = prompt("导入文件（绝对路径）");
  if (!path) return;
  api.importRecording(path)
    .then(() => load())
    .catch((e) => alert(String(e)));
}

onMounted(async () => {
  await load();
  await loadSettings();
  api.screenSize()
    .then(([w, h]) => (screenText.value = w ? `${w}×${h}` : "未知"))
    .catch(() => (screenText.value = "未知"));
  unlistens.push(await onEvent("rec:progress", (p) => (liveCount.value = Number(p))));
  unlistens.push(
    await onEvent("rec:done", () => {
      liveCount.value = 0;
      load();
    }),
  );
  unlistens.push(
    await onEvent<number>("play:countdown", (seconds) => {
      countdown.value = seconds > 0 ? seconds : null;
    }),
  );
  unlistens.push(
    await onEvent("play:loop", (p) => {
      playLoop.value = { done: Number(p), total: repeat.value };
    }),
  );
  unlistens.push(
    await onEvent("play:done", () => {
      countdown.value = null;
      playLoop.value = null;
      playProgress.value = null;
      detailsRef.value?.setCurrentMs(null);
    }),
  );
  unlistens.push(
    await onEvent("play:stopped", () => {
      countdown.value = null;
      playLoop.value = null;
      playProgress.value = null;
      detailsRef.value?.setCurrentMs(null);
    }),
  );
  unlistens.push(
    await onEvent("play:error", (p) => {
      countdown.value = null;
      playLoop.value = null;
      playProgress.value = null;
      detailsRef.value?.setCurrentMs(null);
      alert(String(p));
    }),
  );
  unlistens.push(
    await onEvent("play:progress", (p) => {
      const { index, total } = p as { index: number; total: number };
      playProgress.value = { index, total };
      // 详情弹窗开着：把进度推给弹窗画播放头
      if (details.value && selected.value) {
        // 通过 events 数组下标 + t_ms 计算当前 ms
        const ev = selected.value.events[index - 1];
        if (ev) detailsRef.value?.setCurrentMs(ev.t_ms);
      }
    }),
  );
});

onUnmounted(() => unlistens.forEach((u) => u()));
</script>

<template>
  <div>
    <div class="page-title">操作录制库</div>
    <div class="page-sub">
      按 <span class="hint-kbd">F10</span> 随时开始录制（可先关掉本窗口），再按一次结束并自动保存。
    </div>

    <div class="stats">
      <div class="stat-card">
        <div class="stat-label">脚本数量</div>
        <div class="stat-value">{{ recordings.length }}</div>
      </div>
      <div class="stat-card">
        <div class="stat-label">最近步数</div>
        <div class="stat-value small">{{ lastRec ? lastRec.events.length.toLocaleString() : "—" }}</div>
      </div>
      <div class="stat-card">
        <div class="stat-label">最近时长</div>
        <div class="stat-value small">{{ lastRec ? fmtDuration(lastRec.duration_ms) : "—" }}</div>
      </div>
      <div class="stat-card">
        <div class="stat-label">当前屏幕</div>
        <div class="stat-value small">{{ screenText }}</div>
      </div>
    </div>

    <div class="library-toolbar">
      <input v-model="search" type="search" aria-label="搜索脚本名称或热键" placeholder="搜索脚本名称或热键…" />
      <span aria-live="polite">{{ filteredRecordings.length }} / {{ recordings.length }} 个脚本</span>
      <button v-if="search" class="btn sm ghost" @click="search = ''">清空搜索</button>
      <button class="btn sm" @click="importJson">导入脚本</button>
    </div>

    <div class="rec-grid">
      <div v-if="!recordings.length" class="empty">
        <div class="big">🎬</div>
        还没有录制。<br />按 <span class="hint-kbd">F10</span> 开始第一次录制。
      </div>
      <div
        v-for="rec in filteredRecordings"
        :key="rec.id"
        class="rec-card"
        :class="{ selected: rec.id === selectedId }"
        @click="selectedId = rec.id"
      >
        <div class="rec-icon">▶</div>
        <div class="rec-main">
          <div class="rec-name">
            {{ rec.name }}
            <span v-if="rec.hotkey" class="hk-badge" title="点击修改热键" @click.stop="hotkeyTarget = rec">
              ⚡ {{ prettyHk(rec.hotkey) }}
            </span>
          </div>
          <div class="rec-meta">
            <span>{{ rec.created_at.replace("T", " ") }}</span>
            <span>{{ rec.events.length.toLocaleString() }} 步</span>
            <span>{{ fmtDuration(rec.duration_ms) }}</span>
            <span v-if="rec.screen_width">{{ rec.screen_width }}×{{ rec.screen_height }}</span>
          </div>
        </div>
        <div class="rec-actions" @click.stop>
          <button class="btn sm" @click="details = rec">详情</button>
          <button class="btn sm" @click="editTarget = rec">✏️ 编辑</button>
          <button class="btn sm ghost" @click="hotkeyTarget = rec">⚡ 热键</button>
          <button class="btn sm ghost" @click="rename(rec)">重命名</button>
          <button class="btn sm ghost" :disabled="busy" @click="duplicate(rec)">复制</button>
          <button class="btn sm ghost" @click="exportJson(rec)">导出</button>
          <button class="btn sm danger" @click="remove(rec)">删除</button>
        </div>
      </div>
      <div v-if="recordings.length && !filteredRecordings.length" class="empty">
        没有匹配的脚本，试试其他名称或热键。
      </div>
    </div>

    <div class="status-bar">
      <button class="btn rec-btn" :disabled="playing || busy" @click="toggleRecord">
        {{ recording ? "■ 结束录制（F10）" : "● 开始录制（F10）" }}
      </button>
      <template v-if="recording">
        <span class="rec-dot"></span>
        <span style="color: var(--muted)">正在记录… 已捕获 {{ liveCount.toLocaleString() }} 个事件</span>
      </template>
      <template v-else>
        <span v-if="selected" class="play-target" :title="selected.name">选中：{{ selected.name }}</span>
        <span style="color: var(--muted)">执行参数</span>
        <select v-model.number="speed">
          <option v-for="o in SPEED_OPTIONS" :key="o.value" :value="o.value">{{ o.label }}</option>
        </select>
        <span style="color: var(--muted)">循环</span>
        <input v-model.number="repeat" :disabled="playing" type="number" min="1" max="999" style="min-width: 64px" />
        <span style="color: var(--muted)">等待</span>
        <input v-model.number="waitSeconds" :disabled="playing" type="number" min="0" max="30" style="min-width: 64px" />
        <span style="color: var(--muted)">秒</span>
        <button class="btn primary" :disabled="!selected || playing || busy" @click="play">
          ▶ 执行选中
        </button>
        <button class="btn danger" v-if="playing" @click="stop">■ 紧急停止（Esc）</button>
        <span v-if="playing" style="color: var(--danger)">脚本回放中，任意时刻按 Esc 立即停止</span>
        <span v-if="playLoop" style="color: var(--accent)">第 {{ playLoop.done }} / {{ playLoop.total }} 轮</span>
        <span
          v-if="playProgress"
          style="color: var(--muted); font-variant-numeric: tabular-nums"
        >
          {{ playProgress.index }} / {{ playProgress.total }} 步
          ({{ Math.round((playProgress.index / playProgress.total) * 100) }}%)
        </span>
      </template>
    </div>

    <div v-if="countdown !== null" class="countdown-mask">
      <div class="countdown-num">{{ countdown > 0 ? countdown : "GO" }}</div>
      <button class="btn danger" @click="stop">取消执行（F11）</button>
    </div>

    <DetailsDialog
      v-if="details"
      ref="detailsRef"
      :recording="details"
      @close="details = null"
    />

    <HotkeyDialog
      v-if="hotkeyTarget"
      :name="hotkeyTarget.name"
      :current="hotkeyTarget.hotkey ?? null"
      @close="hotkeyTarget = null"
      @save="saveHotkey"
    />

    <StepEditor
      v-if="editTarget"
      :recording="editTarget"
      @close="editTarget = null"
      @saved="
        editTarget = null;
        load();
      "
    />
  </div>
</template>

<style scoped>
.library-toolbar {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 12px;
  margin: 20px 0 16px;
  color: var(--muted);
  font-size: 13px;
}
.library-toolbar input {
  flex: 1;
  min-width: 180px;
  padding: 9px 12px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--bg);
  color: var(--text);
  font: inherit;
  color-scheme: dark;
}
.library-toolbar input::placeholder { color: var(--muted); }
.library-toolbar input:focus { outline: 2px solid var(--accent); outline-offset: 2px; }
.rec-card { flex-wrap: wrap; }
.rec-main { min-width: 180px; }
.rec-actions { flex-wrap: wrap; justify-content: flex-end; margin-left: auto; }
.play-target { max-width: 180px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.countdown-mask { display: flex; flex-direction: column; justify-content: center; gap: 24px; }
</style>
