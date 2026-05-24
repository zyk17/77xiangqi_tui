# AGENT

本仓库的默认原则：

1. 保持实现小而直接，不为未来假设提前抽象。
2. `engine`, `xiangqi`  和 `book` 允许直接复制 GUI 后端代码过来，再做适配。
3. `ui` 只负责渲染和命中，不直接承载棋规与引擎逻辑。
4. `app` / `game` 负责交互编排与状态汇总。
5. 需要跨模块的业务访问入口，优先收口到薄 `service` 层，不要把逻辑散进 `app`。
6. 内部数据交互统一使用 Rust 内存结构，不做前后端式 JSON 数据流。

## 当前版面定义

- `A`: 棋盘
- `B`: 游戏按钮组
- `C`: 交互命令输入区
- `D`: 实时评估区
- `D` 区始终显示，不因实时评估模式关闭而隐藏
- `D` 区上半部固定 7 项表格，下半部为 PV 列表
- 当前没有计时器区、没有沙盘、没有“查看思考细节”

## 棋盘坐标约定

- **全局唯一 UCI**（红下基准，如炮二平五恒为 `h2e2`）：轴标、输入、状态栏、历史、引擎一致；`rotated` 只翻转棋子显示与 `screen_to_internal` 命中
- 走子输入：`[a-i][0-9][a-i][0-9]`，与当前屏幕标签一致

## 功能全集

- 红 AI
- 黑 AI
- 查询模式
- 新游戏
- 悔棋
- 旋转棋盘
- 上一步
- 下一步
- 复制 FEN
- 粘贴 FEN
- 实时评估

## 命令约定

- 普通输入：UCI/UCCI 着法字符串
- 普通输入格式：`[a-i][0-9][a-i][0-9]`
- Slash 输入：
  - `/new`
  - `/undo`
  - `/prev`
  - `/next`
  - `/rai`
  - `/bai`
  - `/query`
  - `/rotate`
  - `/copyfen`
  - `/pastefen`
  - `/stop`
  - `/help`
  - `/eval`
  - `/exit`
  - `/quit`
- `C` 区是单一输入框，横跨左右区域，不拆独立命令检索面板

## UI 额外要求

- 必须显示上一手走子记号
- 查询模式与自动走子在落子前必须有箭头提示
- 按钮 UI 字体与实现逻辑可完全参考 GUI 仓库 `C:\projects\77xiangqi`
- 按钮应统一走全局封装，不在各块手写散落布局

## 引擎要求

- 当前项目虽移除了智能时间、强制变招等冗余主入口
- 但“额外进程调用”和“流式调用”仍然值得继续参考与复制 GUI 仓库实现
- 查询期间 `pv` 与 `best_move` 都可能变化，TUI 状态层要按流式结果持续更新
- `app/game/ui` 统一使用 `EngineAnalyzeResult` / `AnalysisSnapshot` 等 Rust 结构，不经过 JSON
- 引擎/棋库在 **后台 `std::thread`** 中运行，主循环按需 `draw`；不要用 Tokio 另起一套模型
- 开局库查询走 `service/book_async`（单 worker + 共享 `generation`），勿在主线程同步 `query_opening_book`
- 模式全关时必须 `stop` + `terminate` 引擎子进程，对齐 GUI `clear_engine_mode_state` / `prepare_for_next_engine_command`
- **开局库命中**（`book_blocks_engine`）：本 FEN 下**所有引擎路径均跳过**（infinite、`go`、poll、release 以外的停流）；D 区与箭头用棋库，电脑走 `tick_ai_autoplay_from_book`
- **未命中**：`book_blocks_engine == false` 且 `last_book_fen` 已对齐当前 FEN 后，查询/评估/电脑按原逻辑走引擎
- 棋库判定完成前不挂 infinite；`release_if_idle` 仅在确有子进程时 join/terminate
