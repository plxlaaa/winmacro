# SPEC: 鼠标轨迹 Douglas-Peucker 简化

> 状态：实现中（v2.5 候选） · 目标：把录制的 `mouse_move` 轨迹在不损失视觉精度的前提下压缩 60-80%。

## 1. 目标

鼠标轨迹是录制数据里**体积最大、密度最高、信息最冗余**的部分。当前
`filter::sanitize_events` 只做了"250ms 内同位置去重"（`filter.rs:18`），
录制一段 1 分钟的滑动操作仍可能产生 3-5 万个 `mouse_move` 事件。
后果：
- `recordings.json` 单文件可达 10+ MB
- 启动时全量 parse 慢
- `StepEditor` 列表无阅读性（看不到点击事件）
- 后续"时间轴 UI"上线后轨迹密度淹没其他事件

本特性在录制落盘前对 `mouse_move` 序列做**Douglas-Peucker 折线简化**，
在物理像素级保留视觉路径，把事件数砍到原来的 20-40%。

## 2. 算法

Douglas-Peucker（DP）经典折线简化：
- 输入：有序点序列 `[(x0,y0), (x1,y1), ...]`
- 输出：保留端点 + 转折点的子集
- 递归：以端点连线为基线，找偏离最远的中间点；若最大距离 < ε，则删除所有中间点；否则保留最远点并以它为分界递归两侧

变体（本项目用）：**ε 改用物理像素**（`epsilon_px`），而非相对比例。
理由：宏工具关心"鼠标到底点了哪里"，物理像素误差是用户能感知的唯一标准。

### 2.1 时间维度合并

仅对**连续的 `mouse_move` 序列**做 DP；遇到 `mouse_down` / `mouse_up` /
`mouse_scroll` / `key_*` 等其他事件就断开。简化结果只替换连续段，
**不改变其他事件的 t_ms 排序**。

### 2.2 ε 取档位

| 档位 | ε (px) | 期望压缩率 | 适用 |
|---|---|---|---|
| `off`   | — | 1.00 | 不简化 |
| `low`   | 0.5 | 0.70-0.80 | 美术/像素敏感操作 |
| `medium` | 1.5 | 0.20-0.40 | **默认**，办公自动化 |
| `high`  | 4.0 | 0.10-0.20 | 快速滑动，坐标精度要求低 |

## 3. API 设计

`logic/src/simplify.rs`（新文件）：

```rust
/// 折线简化精度档位。序列化时用字符串 ("off"/"low"/"medium"/"high")，
/// 与 Settings 字段对齐。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SimplifyLevel {
    Off,
    Low,
    #[default]
    Medium,
    High,
}

impl SimplifyLevel {
    pub fn epsilon_px(self) -> Option<f64> {
        match self {
            Self::Off => None,
            Self::Low => Some(0.5),
            Self::Medium => Some(1.5),
            Self::High => Some(4.0),
        }
    }
}

/// 简化一段连续 mouse_move 序列（已按 t_ms 升序）。
/// 非 mouse_move 事件保留原样，不动 t_ms。
/// `level == Off` 或 epsilon 为 None 时等价于克隆输入。
pub fn simplify_moves(
    events: Vec<InputEvent>,
    level: SimplifyLevel,
) -> Vec<InputEvent>;

/// 内部：经典 Douglas-Peucker。
/// 输入输出都是物理坐标点列表。
fn douglas_peucker(points: &[(f64, f64)], epsilon: f64, keep: &mut [bool]);
```

**不变性**：
- 第一个和最后一个 `mouse_move` 永远保留（用于精确锚定点击前后位置）
- 简化前后 `Vec<InputEvent>` 长度 ≤ 原长度
- `kind / vk / key / button / t_ms` 字段除 `t_ms` 不变；`t_ms` 也不变（DP 只动坐标）
- `t_ms` 整体顺序保持递增

## 4. 集成点

| 位置 | 变更 |
|---|---|
| `logic/src/model.rs::Settings` | 加 `#[serde(default)] pub trajectory_simplify: SimplifyLevel`，默认 `Medium` |
| `logic/src/model.rs::Settings::sanitize` | 未知字符串回退为 `Medium` |
| `logic/src/filter.rs::finalize` | 在 `drop_start_release` + `sanitize_events` 之后挂 `simplify_moves(..., simplify_level)`；`Off` 则跳过 |
| `logic/src/lib.rs` | `pub mod simplify;` |
| `app-tauri/src-tauri/src/engine.rs::stop_recording` | 把 `simplify_level` 从 settings 取出，传给 `filter::finalize` |
| `app-tauri/src/views/Settings.vue` | 加单选控件："轨迹精度" 关 / 极致 / 推荐 / 粗略 |
| `app-tauri/src/api.ts` | `Settings` 加 `trajectory_simplify: "off" \| "low" \| "medium" \| "high"` |

## 5. 测试

`logic/src/simplify.rs` 单测：

1. **空输入** → 空输出
2. **单点** → 单点
3. **直线三点** → 保留首尾，中间点删除
4. **直线 100 点** → 仅保留 2 个端点
5. **三角形折点** → 三个顶点都保留
6. **混合事件**（move → down → up → move）→ 两段分别简化，非 move 事件不动
7. **Off 档** → 长度等于原长度
8. **t_ms 顺序保持**：简化后所有事件按 t_ms 严格递增
9. **首尾一定保留**：结果首尾 mouse_move 与输入首尾相同
10. **坐标误差 < ε**：对每个被删除点，构造该段端点直线，垂线距离 < ε
11. **压缩率断言**：1000 个共线点（x, x, x, x...）+ 中间一个偏离 5px 的点：
    - `Low(0.5)` → 中间点保留，压缩率 ≈ 1/1000
    - `High(4.0)` → 中间点也删除，压缩率 ≈ 2/1000
12. **非 move 事件完全不变**（kind / t_ms / x / y / button / vk / key 全部保留）

## 6. UI 草图（Settings.vue）

```
轨迹精度
[ ◯ 关   ● 推荐（1.5 px）   ◯ 极致（0.5 px）   ◯ 粗略（4 px） ]
脚本更精简，回放时鼠标路径视觉无明显差异；不影响点击位置。
```

## 7. 风险与回滚

- **风险 1**：DP 阈值过大可能让"快速划过某按钮"时鼠标路径偏移到按钮外。
  - 缓解：默认 `Medium` 1.5 px，远低于人眼可感差异（4K 屏下 1.5 px ≈ 0.4 mm）。
  - 暴露 `High` 时文档明示"快速滑过场景用"。
- **风险 2**：现有用户的旧 recordings.json 没经过简化，重存时也不会回溯处理。
  - 接受：节省的是新录制场景；旧脚本不受影响。
- **回滚**：`Settings::trajectory_simplify` 默认 `Off` 即可禁用；删除 simplify.rs
  + filter.rs 改动一行 → 完整回滚。

## 8. 不在本次范围

- 路径平滑（曲线拟合）—— DP 折线对鼠标轨迹足够
- 基于时间间隔的自适应 ε —— 增加复杂度，收益边际
- 已有脚本的回溯迁移 —— 见 §7 风险 2
- 时间轴 UI / 智能识别 / 条件节点 —— 这些是另外的 PR
