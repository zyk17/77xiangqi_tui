# Architecture

## 目标

做一个小型、直接、可维护的象棋 TUI。

## 版面

- `A`: 棋盘
- `B`: 游戏按钮组
- `C`: 交互命令输入区
- `D`: 实时评估区

### 棋盘显示约束

- 棋盘最左侧必须标 `0~9`
- 棋盘最下侧必须标 `a~i`
- 必须显示上一手走子记号
- 查询模式与自动走子在实际落子前必须先显示箭头

`C` 区负责两类输入：

1. 直接输入着法字符串，例如 `h2e2`
2. 输入 `/` 命令，例如 `/new`、`/undo`、`/rai`

`C` 区是横跨全宽的单输入框，不再拆独立命令检索面板。

普通着法输入格式固定为：

```text
[a-i][0-9][a-i][0-9]
```

完整命令见 [docs/interaction.md](/C:/projects/77xiangqi_tui/docs/interaction.md)

## 模块职责

当前推荐依赖方向：

```text
app -> service -> engine/book/xiangqi
app -> game
ui  -> app/game
game -> xiangqi
```

原则：

- `app` 薄，只处理事件循环、输入分发、焦点、页面切换
- `service` 薄，但集中承接命令解析、引擎调用、开局库查询、评估更新
- `ui` 不主动“取数”，只渲染当前内存状态
- 不做前后端式 JSON 数据流；统一使用 Rust struct / enum

### `src/xiangqi`

- `u8[90]` 棋盘
- FEN
- 走法与规则
- 胜负判定

### `src/engine`

- UCI / UCCI 协议
- 引擎进程
- 分析结果标准化
- 需要继续参考 GUI 的额外进程调用与流式调用实现

### `src/book`

- 本地库
- 云库
- 命中结果标准化

### `src/game`

- 对局状态
- 历史栈
- 评估汇总
- `best_move` / `pv` / 评估 7 项字段的 TUI 快照
- 上一手与待落子箭头等棋盘提示状态

### `src/service`

- `command`: 输入解析与命令归一化
- `book`: 开局库统一查询入口
- `engine`: 引擎统一调用入口
- `analysis`: 评估快照更新与聚合
- 负责隔离复制来的旧接口和当前 TUI 内存态模型

### `src/app`

- 终端事件循环
- 焦点管理
- 命令输入与提交

### `src/ui`

- ratatui 渲染
- 鼠标点击命中
- 棋盘与表单布局
- 全局按钮封装
- `D` 区 7 项评估表格与 PV 列表渲染

## 迁移策略

- `engine` / `book`：直接从 GUI 后端复制，再拆依赖
- 引擎/开局库结果统一为 Rust struct，经 `service` 收口到 `game` / `ui`
- `xiangqi`：参考现有实现，但按 TUI 需求重写
- `ui` / `game`：独立实现

## UI 参考策略

- 按钮 UI 字体
- 按钮布局
- 按钮实现逻辑

可完全参考 GUI 仓库 `C:\projects\77xiangqi`
