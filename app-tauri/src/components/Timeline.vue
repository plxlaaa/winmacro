<script setup lang="ts">
/**
 * 时间轴组件：在录制详情 / 编辑器 / 主界面顶部展示事件密度。
 *
 * 渲染策略（Canvas，性能比 SVG/DOM 好得多）：
 * - 横轴：t_ms 总时长归一化
 * - 纵轴：4 个泳道：mouse_move / mouse_click / mouse_scroll / key
 * - 每个泳道内按列分桶（≈ 2px 一桶），堆叠该桶内事件数为竖条高度
 * - 鼠标悬停显示该列 t_ms 范围 + 各类型计数
 * - 点击 emit 'seek' 带 t_ms
 * - 外部传入 currentMs 时画红色播放头
 */
import { ref, computed, onMounted, onBeforeUnmount, watch } from "vue";
import type { InputEvent } from "../api";

const props = withDefaults(
  defineProps<{
    events: InputEvent[];
    /** 当前播放位置（毫秒），可选；为 null/undefined 时不画播放头 */
    currentMs?: number | null;
    /** 总时长（毫秒）；不传则从 events 最后一项推导 */
    durationMs?: number;
    /** Canvas 高度（px） */
    height?: number;
  }>(),
  { height: 60, currentMs: null, durationMs: 0 },
);

const emit = defineEmits<{
  (e: "seek", tMs: number): void;
}>();

const canvas = ref<HTMLCanvasElement | null>(null);
const wrap = ref<HTMLDivElement | null>(null);
const hover = ref<{ x: number; tMs: number; counts: Record<string, number> } | null>(
  null,
);

// 4 个泳道的颜色（与 app 主题色一致）
const LANE_COLORS: Record<string, string> = {
  mouse_move: "rgba(96, 165, 250, 0.55)", // 蓝 — 移动
  mouse_click: "rgba(239, 68, 68, 0.85)", // 红 — 点击 (down+up)
  mouse_scroll: "rgba(168, 85, 247, 0.85)", // 紫 — 滚轮
  key: "rgba(34, 197, 94, 0.85)", // 绿 — 键盘
};

function laneOf(ev: InputEvent): string {
  if (ev.kind === "mouse_move") return "mouse_move";
  if (ev.kind === "mouse_scroll") return "mouse_scroll";
  if (ev.kind === "key_down" || ev.kind === "key_up") return "key";
  if (ev.kind === "mouse_down" || ev.kind === "mouse_up") return "mouse_click";
  return "mouse_move"; // 兜底
}

const totalMs = computed(() => {
  if (props.durationMs && props.durationMs > 0) return props.durationMs;
  if (!props.events.length) return 1;
  return Math.max(1, props.events[props.events.length - 1].t_ms);
});

/** 把事件预分桶：{ lane -> bucket -> count }，bucket 索引 = floor(t_ms / bucketMs) */
interface Buckets {
  lane: string;
  data: Uint32Array;
}
let buckets: Buckets[] = [];
let bucketCount = 0;
let bucketMs = 1;

function rebuildBuckets(width: number) {
  bucketCount = Math.max(1, Math.floor(width / 2));
  bucketMs = Math.max(1, Math.ceil(totalMs.value / bucketCount));
  buckets = ["mouse_move", "mouse_click", "mouse_scroll", "key"].map((lane) => ({
    lane,
    data: new Uint32Array(bucketCount),
  }));
  for (const ev of props.events) {
    const lane = laneOf(ev);
    const idx = Math.min(bucketCount - 1, Math.floor(ev.t_ms / bucketMs));
    const b = buckets.find((b) => b.lane === lane);
    if (b) b.data[idx]++;
  }
}

function draw() {
  const c = canvas.value;
  if (!c) return;
  const dpr = window.devicePixelRatio || 1;
  const cssW = c.clientWidth;
  const cssH = c.clientHeight;
  if (cssW < 2) return;
  c.width = Math.floor(cssW * dpr);
  c.height = Math.floor(cssH * dpr);
  const ctx = c.getContext("2d");
  if (!ctx) return;
  ctx.scale(dpr, dpr);
  ctx.clearRect(0, 0, cssW, cssH);

  rebuildBuckets(cssW);

  const lanes = buckets.length;
  const laneH = cssH / lanes;

  // 找每个泳道内的最大计数做归一化
  for (let li = 0; li < lanes; li++) {
    const b = buckets[li];
    let max = 1;
    for (let i = 0; i < b.data.length; i++) {
      if (b.data[i] > max) max = b.data[i];
    }
    ctx.fillStyle = LANE_COLORS[b.lane];
    for (let i = 0; i < b.data.length; i++) {
      const n = b.data[i];
      if (n === 0) continue;
      const h = Math.max(1, (n / max) * laneH * 0.9);
      const x = (i / b.data.length) * cssW;
      const w = cssW / b.data.length;
      ctx.fillRect(x, li * laneH + (laneH - h), w, h);
    }
  }

  // 泳道分隔线
  ctx.strokeStyle = "rgba(255,255,255,0.08)";
  ctx.lineWidth = 1;
  for (let li = 1; li < lanes; li++) {
    ctx.beginPath();
    ctx.moveTo(0, li * laneH);
    ctx.lineTo(cssW, li * laneH);
    ctx.stroke();
  }

  // 播放头
  if (props.currentMs != null && totalMs.value > 0) {
    const x = (props.currentMs / totalMs.value) * cssW;
    ctx.strokeStyle = "#f97316";
    ctx.lineWidth = 2;
    ctx.beginPath();
    ctx.moveTo(x, 0);
    ctx.lineTo(x, cssH);
    ctx.stroke();
  }
}

let resizeObs: ResizeObserver | null = null;

onMounted(() => {
  draw();
  if (wrap.value && "ResizeObserver" in window) {
    resizeObs = new ResizeObserver(() => draw());
    resizeObs.observe(wrap.value);
  }
});
onBeforeUnmount(() => {
  resizeObs?.disconnect();
});

watch(
  () => [props.events.length, props.durationMs, props.currentMs],
  () => draw(),
);

function onMove(e: MouseEvent) {
  const c = canvas.value;
  if (!c) return;
  const rect = c.getBoundingClientRect();
  const x = Math.max(0, Math.min(rect.width, e.clientX - rect.left));
  const tMs = Math.floor((x / rect.width) * totalMs.value);
  const idx = Math.min(bucketCount - 1, Math.floor(tMs / bucketMs));
  const counts: Record<string, number> = {};
  for (const b of buckets) {
    counts[b.lane] = b.data[idx] ?? 0;
  }
  hover.value = { x, tMs, counts };
}
function onLeave() {
  hover.value = null;
}
function onClick(e: MouseEvent) {
  const c = canvas.value;
  if (!c) return;
  const rect = c.getBoundingClientRect();
  const x = Math.max(0, Math.min(rect.width, e.clientX - rect.left));
  const tMs = Math.floor((x / rect.width) * totalMs.value);
  emit("seek", tMs);
}
</script>

<template>
  <div ref="wrap" class="timeline">
    <canvas
      ref="canvas"
      class="timeline-canvas"
      :style="{ height: height + 'px' }"
      @mousemove="onMove"
      @mouseleave="onLeave"
      @click="onClick"
    />
    <div v-if="hover" class="timeline-tip" :style="{ left: hover.x + 'px' }">
      <div class="tip-time">@ {{ (hover.tMs / 1000).toFixed(2) }}s</div>
      <div v-if="hover.counts.mouse_move" class="tip-row">
        <span class="dot" style="background: rgba(96, 165, 250, 0.85)"></span>
        移动 {{ hover.counts.mouse_move }}
      </div>
      <div v-if="hover.counts.mouse_click" class="tip-row">
        <span class="dot" style="background: rgba(239, 68, 68, 0.85)"></span>
        点击 {{ hover.counts.mouse_click }}
      </div>
      <div v-if="hover.counts.mouse_scroll" class="tip-row">
        <span class="dot" style="background: rgba(168, 85, 247, 0.85)"></span>
        滚轮 {{ hover.counts.mouse_scroll }}
      </div>
      <div v-if="hover.counts.key" class="tip-row">
        <span class="dot" style="background: rgba(34, 197, 94, 0.85)"></span>
        键盘 {{ hover.counts.key }}
      </div>
    </div>
    <div v-if="!events.length" class="timeline-empty">空录制</div>
  </div>
</template>

<style scoped>
.timeline {
  position: relative;
  width: 100%;
  background: var(--bg);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  overflow: hidden;
}
.timeline-canvas {
  display: block;
  width: 100%;
  cursor: crosshair;
}
.timeline-empty {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--muted);
  font-size: 12.5px;
}
.timeline-tip {
  position: absolute;
  top: 4px;
  transform: translateX(-50%);
  background: rgba(0, 0, 0, 0.82);
  color: #fff;
  padding: 5px 8px;
  border-radius: 4px;
  font-size: 11.5px;
  white-space: nowrap;
  pointer-events: none;
  z-index: 5;
}
.tip-time {
  font-weight: 600;
  margin-bottom: 2px;
  color: #f97316;
}
.tip-row {
  display: flex;
  align-items: center;
  gap: 5px;
  font-size: 11px;
  line-height: 1.5;
}
.dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  display: inline-block;
}
</style>
