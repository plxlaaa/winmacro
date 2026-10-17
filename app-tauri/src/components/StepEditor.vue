<script setup lang="ts">
import { ref, computed } from "vue";
import {
  api,
  KIND_LABELS,
  BUTTON_LABELS,
  fmtDuration,
  type Recording,
  type InputEvent,
} from "../api";

const props = defineProps<{ recording: Recording }>();
const emit = defineEmits<{ (e: "close"): void; (e: "saved"): void }>();

// ── 草稿副本：所有编辑作用于本地拷贝，保存时整体提交 ──
const events = ref<InputEvent[]>(
  props.recording.events.map((e) => ({ ...e })),
);
const original = JSON.stringify(props.recording.events);
const dirty = computed(() => JSON.stringify(events.value) !== original);
const saving = ref(false);

// ── 视图：默认隐藏 mouse_move（轨迹噪音），只看关键步骤 ──
const hideMoves = ref(true);
const PAGE = 300;
const page = ref(0);

const rows = computed(() => {
  const all = events.value.map((ev, i) => ({ ev, i }));
  return hideMoves.value
    ? all.filter((r) => r.ev.kind !== "mouse_move")
    : all;
});
const shown = computed(() => rows.value.slice(0, page.value * PAGE + PAGE));
const hasMore = computed(() => shown.value.length < rows.value.length);

// ── 选择（基于真实下标，切换过滤不丢选择）──
const selected = ref(new Set<number>());
function toggleSel(i: number) {
  const s = new Set(selected.value);
  s.has(i) ? s.delete(i) : s.add(i);
  selected.value = s;
}
const selCount = computed(() => selected.value.size);

// ── 编辑操作 ──
function gapOf(i: number): number {
  const prev = events.value[i - 1];
  return prev ? events.value[i].t_ms - prev.t_ms : events.value[i].t_ms;
}

/** 修改第 i 步的间隔：该步及后续整体平移，保持后续节奏 */
function setGap(i: number, gap: number) {
  const g = Math.max(0, Math.min(600000, Math.round(gap)));
  const delta = g - gapOf(i);
  if (delta === 0) return;
  for (let j = i; j < events.value.length; j++) {
    events.value[j].t_ms = Math.max(0, events.value[j].t_ms + delta);
  }
}

/** 修改鼠标步骤坐标 */
function setPos(i: number, axis: "x" | "y", val: number) {
  const ev = events.value[i];
  const v = Math.max(0, Math.min(32767, Math.round(val)));
  ev[axis] = v;
}

/** 删除选中步骤；剩余步骤保持原绝对时间（等待时长不压缩） */
function deleteSelected() {
  if (!selCount.value) return;
  if (!confirm(`删除选中的 ${selCount.value} 个步骤？`)) return;
  const s = selected.value;
  events.value = events.value.filter((_, i) => !s.has(i));
  selected.value.clear();
  page.value = 0;
}

/** 裁剪：只保留 [first, last] 闭区间的真实下标，首步时间归零 */
function trimSelected() {
  const s = [...selected.value].sort((a, b) => a - b);
  if (s.length < 2) {
    alert("请先勾选 ≥2 个步骤作为裁剪的起止范围");
    return;
  }
  const start = s[0];
  const end = s[s.length - 1];
  if (!confirm(`只保留第 ${start + 1} ~ ${end + 1} 步（共 ${end - start + 1} 步）？`)) return;
  const kept = events.value.slice(start, end + 1);
  const t0 = kept[0].t_ms;
  for (const ev of kept) ev.t_ms = Math.max(0, ev.t_ms - t0);
  events.value = kept;
  selected.value.clear();
  page.value = 0;
}

function resetDraft() {
  events.value = (JSON.parse(original) as InputEvent[]).map((e) => ({ ...e }));
  selected.value.clear();
  page.value = 0;
}

const durationMs = computed(() =>
  events.value.length ? events.value[events.value.length - 1].t_ms : 0,
);

async function save() {
  if (!events.value.length) {
    alert("至少保留一个步骤");
    return;
  }
  saving.value = true;
  try {
    await api.updateRecordingEvents(props.recording.id, events.value);
    emit("saved");
  } catch (e) {
    alert(String(e));
  } finally {
    saving.value = false;
  }
}

function extraText(ev: InputEvent): string {
  if (ev.kind === "mouse_scroll") {
    const dy = ev.dy ?? 0;
    return `滚动 ${dy > 0 ? "↓" : "↑"} ${Math.abs(dy)}`;
  }
  if (ev.kind === "mouse_down" || ev.kind === "mouse_up")
    return BUTTON_LABELS[ev.button ?? "left"] ?? ev.button ?? "—";
  if (ev.kind === "key_down" || ev.kind === "key_up") return ev.key ?? "—";
  return "—";
}
</script>

<template>
  <div class="dialog-mask">
    <div class="dialog step-editor">
      <div class="dialog-head">
        <div>
          <div class="dialog-title">编辑步骤 — {{ recording.name }}</div>
          <div class="dialog-sub">
            {{ events.length.toLocaleString() }} 步 · {{ fmtDuration(durationMs) }}
            <span v-if="dirty" style="color: var(--danger)">● 有未保存修改</span>
          </div>
        </div>
        <button class="btn" @click="emit('close')">关闭</button>
      </div>

      <div class="editor-toolbar">
        <label class="switch" title="轨迹移动事件通常不需要逐条查看">
          <input v-model="hideMoves" type="checkbox" @change="page = 0" />
          <span class="track"></span>
          <span class="thumb"></span>
        </label>
        <span style="color: var(--muted); font-size: 12.5px">隐藏鼠标轨迹</span>
        <span style="flex: 1"></span>
        <button class="btn sm danger" :disabled="!selCount" @click="deleteSelected">
          删除选中（{{ selCount }}）
        </button>
        <button class="btn sm" :disabled="!selCount" @click="trimSelected">
          裁剪到选中范围
        </button>
        <button class="btn sm ghost" :disabled="!dirty" @click="resetDraft">
          撤销全部修改
        </button>
        <button class="btn sm primary" :disabled="!dirty || saving" @click="save">
          {{ saving ? "保存中…" : "保存修改" }}
        </button>
      </div>

      <div class="steps-wrap">
        <table class="steps editor-table">
          <thead>
            <tr>
              <th style="width: 34px"></th>
              <th style="width: 64px">#</th>
              <th style="width: 86px">类型</th>
              <th style="width: 110px">间隔(ms)</th>
              <th style="width: 200px">坐标</th>
              <th>详情</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="{ ev, i } in shown"
              :key="i"
              :class="{ selrow: selected.has(i) }"
            >
              <td><input type="checkbox" :checked="selected.has(i)" @change="toggleSel(i)" /></td>
              <td class="idx">{{ i + 1 }}</td>
              <td>{{ KIND_LABELS[ev.kind] ?? ev.kind }}</td>
              <td>
                <input
                  class="cell-input num"
                  type="number"
                  min="0"
                  :value="gapOf(i)"
                  @change="setGap(i, Number(($event.target as HTMLInputElement).value))"
                />
              </td>
              <td>
                <template v-if="ev.x != null && ev.y != null">
                  <input
                    class="cell-input num"
                    type="number"
                    min="0"
                    :value="ev.x"
                    @change="setPos(i, 'x', Number(($event.target as HTMLInputElement).value))"
                  />
                  <span class="pos-sep">,</span>
                  <input
                    class="cell-input num"
                    type="number"
                    min="0"
                    :value="ev.y"
                    @change="setPos(i, 'y', Number(($event.target as HTMLInputElement).value))"
                  />
                </template>
                <span v-else>—</span>
              </td>
              <td class="detail-cell">{{ extraText(ev) }}</td>
            </tr>
          </tbody>
        </table>
        <div v-if="hasMore" style="text-align: center; padding: 14px">
          <button class="btn" @click="page++">
            加载更多（已显示 {{ shown.length }} / {{ rows.length }}）
          </button>
        </div>
        <div v-if="!rows.length" class="empty" style="padding: 40px">
          没有可显示的步骤
        </div>
      </div>

      <div class="editor-foot">
        间隔 = 该步相对上一步的等待毫秒（首步为起始等待）；修改会平移后续步骤节奏。
        删除步骤不压缩剩余等待时间。坐标限 0 ~ 32767。
      </div>
    </div>
  </div>
</template>

<style scoped>
.step-editor {
  width: min(920px, 94vw);
  display: flex;
  flex-direction: column;
  max-height: 88vh;
}
.editor-toolbar {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 0 12px;
  flex-wrap: wrap;
}
.steps-wrap {
  overflow-y: auto;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
}
.editor-table td {
  padding: 3px 8px;
}
.editor-table tr.selrow {
  background: var(--accent-soft);
}
.cell-input {
  width: 76px;
  background: var(--bg);
  border: 1px solid var(--border);
  border-radius: 4px;
  color: var(--text);
  padding: 2px 6px;
  font-family: Consolas, monospace;
  font-size: 12.5px;
}
.cell-input:focus {
  outline: none;
  border-color: var(--accent);
}
.pos-sep {
  color: var(--muted);
  margin: 0 3px;
}
.detail-cell {
  color: var(--muted);
  font-size: 12.5px;
  user-select: text;
}
.editor-foot {
  margin-top: 10px;
  color: var(--faint);
  font-size: 11.5px;
  line-height: 1.6;
}
</style>
