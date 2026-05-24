# Architecture

## 目标

小型、直接、可维护的象棋 TUI；行为与 GUI 对弈页对齐，避免在 TUI 内另起一套异步或数据模型。

## 版面

- `A`: 棋盘
- `B`: 游戏按钮组
- `C`: 交互命令输入区
- `D`: 实时评估区（始终显示；关闭实时模式时内容可为 idle）

棋盘约束、命令表、终局行为见 [interaction.md](interaction.md)。

## 模块职责

推荐依赖方向：

```text
app -> service -> engine / book / xiangqi
app -> game
ui  -> app / game（只读渲染）
game -> xiangqi
```

原则：

- `app`：事件循环、焦点、模式开关、调度 `tick_*`，不堆协议细节
- `service`：命令解析、引擎/棋库/分析收口
- `ui`：只渲染当前内存状态，不主动拉引擎
- 模块间用 Rust struct / enum，不做 JSON 桥接

### `src/xiangqi`

- `u8[90]` 棋盘、FEN、规则、胜负
- **全局 UCI** 坐标；`rotated` 仅影响显示与屏幕命中映射

### `src/engine`

- UCI/UCCI 子进程、`EngineAnalysisStore` 流式快照
- `EngineStreamRuntime`：后台 `go infinite` 与 AI `go movetime` 互斥
- 停流时向引擎发 `stop` 并 join；无消费者时 `terminate` 子进程

### `src/book`

- 本地 OBK、云库、`query_opening_book`

### `src/game`

- 对局、历史栈、评估快照、选子/箭头/上一手

### `src/service`

| 模块 | 职责 |
|------|------|
| `command` | 着法与 Slash 解析 |
| `engine` | `EngineService` → `EngineStreamRuntime` |
| `engine_policy` | 是否挂 `go infinite`（对齐 GUI `shouldAttachInfiniteStreamPlay`） |
| `analysis` | 引擎/棋库结果写入 `AnalysisSnapshot` |
| `autoplay` | AI 阶段、箭头、棋库展示应用 |
| `book_async` | 开局库 **单 worker** 队列；`generation` 与主线程共享，避免堆积 join 线程 |

### `src/app`

- `run()`：`poll` → `tick_book_queries` / `tick_engine_stream` / `tick_ai_autoplay` → **按需** `draw`
- `sync_engine_lifecycle`：无模式需要时释放引擎进程

### `src/ui`

- 棋盘网格、按钮面板、D 区表格与 PV、设置页、帮助浮层

## 并发模型（非 async/await）

```text
主线程                     后台线程
────────                   ────────
poll 输入 ~16ms            engine: go infinite / go movetime
tick → 读 store revision   book_async: query_opening_book
dirty → terminal.draw()    engine: stdout 读行（已有）
```

- 不使用 Tokio；与 GUI Tauri 后端 `thread::spawn` + `Mutex` 一致。
- D 区数值刷新节流约 **200ms**（`EVAL_PANEL_REFRESH_MS`）；棋盘重绘不与此绑定。

## 迁移与参考

- `engine` / `book`：从 GUI 后端复制后做 TUI 适配
- 模式停机、infinite 策略、双电脑+仅实时评估不挂流等：对照 `C:\projects\77xiangqi` 前端 `playEngine*` 与 `tauri_backend` `engine_runtime`
