# 77 Xiangqi TUI

这是一个小巧的 `ratatui` 中国象棋终端应用。

当前仓库已经完成第一阶段初始化：

- Cargo 工程已创建
- Git 仓库已初始化
- `src` 模块骨架已建立
- 最小 TUI 已可编译运行
- 已建立薄 `service` 层骨架
- 布局已按最新定义修正：
  - `A`: 棋盘
  - `B`: 游戏按钮组
  - `C`: 交互命令输入区
  - `D`: 实时评估区

## 当前交互定义

- `A` 棋盘必须标注坐标系：
  - 最左侧标 `0~9`
  - 最下侧标 `a~i`
- 普通输入支持所有四字符 UCI/UCCI 坐标着法：
  - 格式：`[a-i][0-9][a-i][0-9]`
  - 示例：`h2e2`、`h0g2`
- `C` 区是单一输入框：
  - 横跨左右区域
  - 不拆独立命令检索面板
  - 普通着法与 Slash 命令都从这里输入
- `D` 区是实时评估区，不是命令区
- `D` 区上半部固定为 7 项表格：
  - 用时
  - 深度
  - NPS
  - 节点
  - 分数
  - 推荐
  - 红/黑胜率
- `D` 区下半部为 PV 列表
  - 单条 PV 最多展示 16 步
  - 查询期间 `pv` 与 `best_move` 都可能持续变化
- 内部数据交互不再走 JSON：
  - `app/game/ui/service` 间统一使用 Rust struct / enum
  - UI 直接读取内存中的最新状态驱动刷新
- 当前开关模式：
  - 红 AI
  - 黑 AI
  - 查询模式
  - 实时评估
- UI 需要显示“上一手走子记号”
- 查询模式与自动走子在真正落子前，需要先显示箭头提示
- 当前没有计时器区、没有沙盘、没有“查看思考细节”

## 命令大全

- 普通着法输入：
  - `[a-i][0-9][a-i][0-9]`
- Slash 命令：
  - `/new`：新游戏
  - `/undo`：悔棋
  - `/prev`：上一步
  - `/next`：下一步
  - `/rai`：红 AI 开关
  - `/bai`：黑 AI 开关
  - `/query`：查询模式开关
  - `/rotate`：旋转棋盘
  - `/copyfen`：复制 FEN
  - `/pastefen`：粘贴 FEN
  - `/eval`：实时评估开关
  - `/exit`：退出软件
  - `/quit`：退出软件

## 运行

```bash
cargo run
```

## 当前交互

- `Tab` / `Shift+Tab`：切换焦点
- 方向键 / `hjkl`：移动焦点
- `Enter`：提交命令
- 在 `C` 区可直接输入：
  - 着法字符串，如 `h2e2`
  - Slash 命令，如 `/new`、`/undo`、`/query`、`/rotate`、`/eval`、`/exit`

## 模块

```text
src/
  app/       事件循环、命令输入、页面切换
  book/      开局库接入
  engine/    UCI/UCCI 接入
  game/      对局状态与评估聚合
  service/   命令、分析、开局库、引擎调用收口
  ui/        ratatui 渲染
  xiangqi/   u8[90] 棋盘核心
docs/
  architecture.md
```

## 参考仓库

GUI 仓库在 `C:\projects\77xiangqi`。

后续策略：

- `engine`、`book`：直接复制 GUI 后端相关实现再做 TUI 适配
- `service`：承接业务访问入口，避免把协议/查询/命令细节堆进 `app`
- 引擎额外进程调用与流式调用仍然值得参考 GUI 仓库，不应因为当前精简而放弃
- `xiangqi`：围绕 `u8[90]` 重写
- `ui`、`game`：按 TUI 交互模型重做
- 按钮 UI 字体与交互逻辑可直接参考 `C:\projects\77xiangqi`
