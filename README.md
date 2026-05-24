# 77 Xiangqi TUI

77象棋TUI 是一个基于 [ratatui](https://github.com/ratatui/ratatui) 的中国象棋终端客户端，引擎与开局库实现对齐 GUI 仓库 [`77xiangqi`](C:\projects\77xiangqi)，数据在进程内用 Rust 结构传递，不经 JSON。

## 功能概览

| 区域 | 说明 |
|------|------|
| **A** 棋盘 | 坐标轴、上一手记号、选子/光标网格高亮、待走箭头 |
| **B** 按钮 | 红/黑电脑、查询、实时评估、新局、悔棋、步进、FEN、旋转 |
| **C** 命令 | 着法 `h2e2`、Slash 命令、Tab 补全 |
| **D** 评估 | 7 项统计表 + PV（流式更新，约 200ms 节流） |

- **红/黑电脑**：后台 `go` 自动走子，思考中更新箭头与 D 区
- **查询 / 实时评估**：后台 `go infinite` 共享分析流（策略对齐 GUI `shouldAttachInfiniteStreamPlay`）
- **开局库**：本地/云库异步查询（单 worker），命中可展示或阻挡引擎
- **终局**：自动停止模式与引擎；可浏览棋谱，`/new` 开新局

## 构建与运行

```bash
cargo run
```

发布构建（`Cargo.toml` 已配置 `lto` / `strip`）：

```bash
cargo build --release
```

### 质量检查

```bash
cargo test
cargo lint-strict    # 或: cargo check 后见 .cargo/config.toml 别名
```

Windows 一键：

```powershell
.\scripts\check.ps1
```

### 环境变量

| 变量 | 说明 |
|------|------|
| `XIANGQI_ENGINE_PATH` | 默认引擎可执行文件路径（亦可在设置页配置） |
| `XIANGQI_TUI_DEBUG=1` | 写入 `logs/runtime.log` 调试日志 |

## 操作

### 焦点与页面

- **Tab**：对弈 ↔ 设置（命令框内 Tab 为 Slash 补全）
- **顶栏**：鼠标点「对弈 / 设置」
- **?** 或 **`/help`**：操作说明浮层（Esc 关闭）
- **Ctrl+C**：退出

### 棋盘（焦点在 A 时）

- **方向键**：移动光标（**全局 UCI** 坐标，与 `rotated` 无关）
- **空格**：选己方子 / 点目标格走子；再点己方子可改选
- **:** 或 **/**：进入 C 区命令输入

### 着法与命令

- 着法：`[a-i][0-9][a-i][0-9]`，例如 `h2e2`
- Slash 命令见下表（在 C 区输入，Enter 提交）

| 命令 | 作用 |
|------|------|
| `/new` | 停止全部模式并开新局 |
| `/stop` | 停止电脑/查询/实时/引擎；**不改变**当前棋谱 |
| `/undo` | 悔棋 |
| `/prev` `/next` | 浏览棋谱 |
| `/rai` `/bai` | 红/黑电脑开关 |
| `/query` `/eval` | 查询模式 / 实时评估开关 |
| `/rotate` | 旋转棋盘显示 |
| `/copyfen` | 复制 FEN 到剪贴板 |
| `/pastefen <FEN>` | 粘贴 FEN（可含空格，如 `w - - 0 1`） |
| `/help` | 帮助浮层 |
| `/exit` `/quit` | 退出 |

## 架构要点

- **主线程**：`crossterm` 事件轮询（约 16ms）；仅在输入、分析更新、AI/棋库状态变化等 **dirty** 时全屏重绘，空闲时不刷 60fps。
- **后台线程**：引擎 `go infinite` / AI `go`、开局库查询；通过 `Arc<Mutex<EngineAnalysisStore>>` 等与主线程交换快照。
- **不使用 tokio**：UCI 阻塞协议 + 同步 TUI，与 GUI Rust 后端「线程 + 锁」模型一致。

模块与依赖见 [docs/architecture.md](docs/architecture.md)，交互细则见 [docs/interaction.md](docs/interaction.md)，协作约定见 [AGENT.md](AGENT.md)。

## 目录结构

```text
src/
  app/              事件循环、模式调度、引擎/棋库 tick
  book/             开局库（本地 OBK / 云库）
  engine/           UCI/UCCI 进程与流式分析
  game/             对局状态、历史、评估快照
  service/          command / analysis / autoplay / engine / book_async
  ui/               ratatui 渲染与命中
  xiangqi/          棋盘、规则、FEN、UCI
docs/
  architecture.md
  interaction.md
scripts/
  check.ps1         测试 + lint-strict
```

## 参考仓库

GUI 对弈页实现：`C:\projects\77xiangqi`（Tauri 后端引擎 supervisor、流式分析、模式停机逻辑为主要参考）。
