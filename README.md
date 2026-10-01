# slint-ms-calculator — 复刻 Windows 计算器计划

目标：用 **Slint + Rust** 复刻 [Microsoft Calculator](https://github.com/Microsoft/calculator) 的**标准型模式**，
含历史记录面板、记忆条（MS/M+/M-/MR/MC）、双行显示、键盘输入；界面按 **Win11 浅色 Fluent** 风格还原。
架构上为后续「科学型 / 程序员模式」预留扩展点。

范围决策（已确认）：
- ✅ 第一版：标准型 + 历史记录 + 记忆条 + 键盘输入
- ⏭️ 预留：科学型、程序员模式、换算（模式切换 UI 先占位）

---

## 1. 与原版的功能对照

| 原版特性 | 第一版是否复刻 | 说明 |
|---|---|---|
| 双行显示（小字公式 / 大字结果） | ✅ | 输入时公式行实时显示表达式，结果行大字 |
| `2 + 3 × 4 = 14`（**运算优先级**） | ✅ | 与现有 slint-calculator 左到右求值本质不同，需表达式引擎 |
| 百分号 `50+10% = 55`（上下文语义） | ✅ | `A + B%` = `A + A×B/100`，非简单除百 |
| 除以 0 → `除数不能为零。` | ✅ | 错误文案本地化 |
| 数字分组 `1,234,567`、科学计数法 | ✅ | 格式化模块 |
| `±` 正负切换、`1/x`、`x²`、`√x` | ✅ | 一元运算，作用于当前输入值 |
| 退格 / C / CE（CE=只清当前输入） | ✅ | 原版同时有 C 和 CE |
| 记忆条 MS、M+、M-、MR、MC + M 徽标 | ✅ | 含"已存记忆"弹出提示 |
| 历史面板（右侧列表、点击回填、清空） | ✅ | Slint `ListView` + `VecModel` |
| 键盘输入（数字、`+-*/`、Enter、Backspace、Esc=F9?、Ctrl+H 切历史） | ✅ | `FocusScope` key-pressed |
| 小键盘布局 `±  ←  C  CE  ` 顶行 | ✅ | Win11 标准键盘排布 |
| 汉堡菜单 + 模式切换页 | 占位 | 只做 UI 骨架，列表含"科学/程序员"灰项 |
| 科学型 / 程序员 / 换算 | ❌ 后续 | 引擎接口设计成可插函数表即可支持 |

Win11 标准键盘布局（5 行）：

```
%   CE   C   ⌫
⅟x   x²   √x   ÷
7    8    9    ×
4    5    6    −
1    2    3    +
±    0    .    =
```

## 2. 总体架构

```
┌─ ui.slint（纯视图）────────────────────────┐
│ MainWindow                                  │
│ ├ 显示屏（expression-line / result-line）    │
│ ├ 记忆条（MemoryPanel）      ←┐             │
│ ├ 键盘（KeyGrid + KeypadKey） │ 视图组件     │
│ └ 历史侧栏（HistoryList）    ←┘             │
└──────────────┬──────────────────────────────┘
        属性 in/out + callback（唯一接口层）
┌──────────────┴──────────────────────────────┐
│ AppController（Rust，src/app.rs）            │
│  - CalcState：输入态、表达式态、结果态         │
│  - 按键/键盘事件 → 状态迁移 → 刷新 UI 属性     │
├─────────────────────────────────────────────┤
│ engine（Rust 纯逻辑，src/engine/，零依赖 Slint）│
│  lexer → parser(AST) → eval(Rational)        │
│  format（分组/精度/科学计数法）                │
│  history.rs（VecModel<HistoryItem>）          │
│  memory.rs（记忆值）                          │
└─────────────────────────────────────────────┘
```

原则：
- **engine 不 import slint** —— 纯 Rust 模块，单元测试无 GUI；UI 适配层单独薄薄一层
- 表达式求值 = 调度场算法或递归下降，操作符优先级 `× ÷` > `+ −`
- 数值类型用 **`Rational`（i64 分子/分母）** 复刻原版的精确十进制行为（`0.1+0.2=0.3` 而非 0.30000000000000004）；溢出时降级 f64 并转科学计数法

## 3. 项目结构

```
slint-ms-calculator/
├── Cargo.toml               # slint, slint-build; dev: 无需额外
├── build.rs
├── ui/
│   ├── main.slint           # MainWindow + 组合
│   ├── keypad.slint         # KeypadKey 组件 + KeyGrid
│   ├── display.slint        # 双行显示 + 错误文案
│   ├── memory.slint         # 记忆条
│   ├── history.slint        # 历史侧栏（ListView）
│   ├── scientific.slint     # 科学键盘 + 分组折叠
│   ├── settings.slint       # 设置页（主题/关于）
│   ├── icons/               # 图标资源：27 个单色 SVG（Tabler，MIT）+ 应用图标 app-calculator.svg
│   │                        #   （icon-abacus.png 是改造前的彩色算盘位图，留作历史参考）
│   └── theme.slint          # 全局常量：色板/圆角/字号（仿 Fluent Light）
├── src/
│   ├── main.rs              # 入口：创建窗口 + 接线
│   ├── app.rs               # AppController：状态机与 UI 属性映射
│   └── engine/
│       ├── mod.rs
│       ├── lexer.rs         # Token：数字/运算符/一元函数/括号(预留)
│       ├── parser.rs        # 优先级爬升 → AST
│       ├── value.rs         # Rational 四则 + 转换 + 溢出策略
│       ├── eval.rs          # 求值 + 错误类型（DivByZero/Overflow/Invalid）
│       ├── format.rs        # 千分组、有效数字(16位)、科学计数法
│       ├── history.rs       # HistoryEntry{expression,result}；上限 100 条
│       └── memory.rs        # MemStore：MS/M+/M-/MR/MC + busy 标志
├── tests/
│   └── engine_tests.rs      # 引擎表驱动测试（对照原版行为表）
└── docs/
    └── 复刻计划.md           # 本文件
```

主题常量（仿 Win11 浅色）：窗口 `#f3f3f3`，卡片/按键 `#fbfbfb`，按键 hover `#f5f5f5` 按下 `#e8e8e8`，文字 `#1a1a1a` 次要 `#606060`，强调 `#0067c0`，圆角 `4px`，主结果字号 `40px`。

## 3.1 运行
```bash
# Windows
cargo run
cargo build --release

# Linux（X11 / Wayland 均可，本机实测 2026-10-01）
cargo run                 # 调试运行，窗口事件循环正常
cargo build --release
cargo test                # lib 31 passed + 引擎集成 21 passed
# 交叉检查 Windows 分支仍能通过类型检查：
cargo check --target x86_64-pc-windows-msvc

# 系统依赖：链接期需要 fontconfig / freetype（Debian/Ubuntu: libfontconfig-dev libfreetype-dev）
# 运行时可选：xdg-open（设置页打开链接）、gsettings（浅色/深色自动跟随，缺失则按浅色）
```

平台差异都收在 `src/main.rs` 的 `mod platform` 里，按 `#[cfg(windows)]` / `#[cfg(not(windows))]` 隔离：
Windows 走 `dwmapi`/`user32` 设圆角、读注册表 `AppsUseLightTheme`、用 `cmd /C start` 开链接；
Linux 深浅色读 `gsettings org.gnome.desktop.interface color-scheme`、链接用 `xdg-open`；
macOS 深浅色读 `defaults read -g AppleInterfaceStyle`、链接用 `open`。
`force_rounded_corners()` 只有 Windows 有实现（其余平台是空函数）——非 Windows 的置顶模式圆角改由
`.slint` 自绘（见 3.4）。置顶切换后的 2px 尺寸微扰（winit 不发 resize 事件）三平台共用，故保留在通用代码里。

## 3.2 Linux 界面实测（2026-10-01，Ubuntu / GNOME Shell 50，Wayland 会话内抓 XWayland 窗口）

抓图通路：把 `WAYLAND_DISPLAY` 置空让 winit 走 X11，再用 `XGetImage` 直接抓窗口客户区。
整屏抓不到（合成器接管 root 后抓 root 是黑的，ffmpeg `x11grab` 实测只有 344 个非黑像素；
GNOME 50 的 `org.gnome.Shell.Screenshot` D-Bus 对普通进程返回 `Screenshot is not allowed`；本机没有 grim/spectacle）。
窗口按 `_NET_WM_PID` 认领，否则会抓到上一次没退干净的实例（踩过：五张状态图字节完全相同）。

| 界面 | 中文 | 彩色 emoji | 备注 |
|---|---|---|---|
| 标准键盘 380×580 | 正常（三 / 标准） | 📌 正常 | 深色自动跟随 `color-scheme='prefer-dark'` |
| 科学键盘 380×620 | 正常（三角学 / 函数） | 🔽 正常 | x²、²√x、xʸ、10ˣ 上下标正常 |
| ≡ 模式菜单 | 正常 | 🧮📅 正常 | 2 组标题 + 18 项，`Flickable content-height: 724px`，窗口内可见到「速度」行和底部「设置」，数据/压强/角度 在折叠线以下，靠滚轮翻 |
| 历史面板 | 正常（无历史记录） | 🗑 单色描边 | |
| 设置页 | 正常 | 🎨 正常 | |

渲染器差异（emoji 版图标缺失的唯一成因）：`SLINT_BACKEND=software` 强制软件渲染器时，彩色 emoji
（🧮🧪📅💱🏃🕐）整块渲染为空白，只有在单色符号字体里查得到的（📈📦📏🌡）还画得出来；
默认（femtovg/GL）三列全部正常。所以「图标不显示」只会出现在 GL 不可用、Slint 退回软件渲染器的机器上
（无 3D 驱动的虚拟机、部分远程会话）。同一张菜单图，software / femtovg / 默认三份抓图两两 md5 不同。
上表是改造前 emoji 版的实测，保留作对照；图标现已全部换成单色 SVG，见 3.3。

置顶（无边框）模式尺寸：修复前实测切换后变成 320×480（= `min-width`×`min-height`）或 380×617
（X11 去掉标题栏后把 37px 算进客户区），且两次运行结果不一致；修复后以 `pin-toggled` 回调入口量到的
真实尺寸为还原目标（入口实测 380×580，120ms 后已是坏值，不能当基准），连测三次稳定 380×580，
release 构建同样 380×580。±2px 两步微扰仍保留（Windows 靠它触发 WM_SIZE 重排）。
已知边界：微扰占用约 240ms，这期间切「标准↔科学」会被还原值覆盖（实测 `pin,sci` 停在 580 而非 620），人手节奏碰不到。

圆角：当时记的结论"应用侧要圆角只能自绘，而 Slint 的 winit 后端没有稳定的透明窗口支持，故未做"是错的——
winit 后端建窗时本来就带 `with_transparent(true)`，自绘圆角可行，实现与实测见 3.4。

dev 与 release 抓图字节一致（`/tmp/s_drawer.png` 与 `/tmp/rel_drawer.png` md5 相同）；
去掉验证脚手架后的抓图与修复前基线 md5 相同（`d184382f…`），确认清理没改渲染。

## 3.3 图标改为单色 SVG 资源（2026-10-01）

3.2 的结论是：只要 Slint 退回软件渲染器，emoji 图标就成块空白。故把 29 处 emoji 文本节点全部换成
`ui/icons/` 下的 27 个单色 SVG（Tabler Icons v3.48.0，MIT，24×24 线性、`stroke="currentColor"`）。
规范化只做两件事：删掉源文件里 `stroke="none"` 的占位 path、去掉 `width/height/class` 属性
（尺寸由 `.slint` 里的 `Image` 决定，不留两处真值）。取包走
`unpkg.com/@tabler/icons@3.48.0/icons/outline/<name>.svg`（本机 crates.io 403，unpkg / jsdelivr 200）。

Slint 侧两个坑：
- `@image-url()` 必须是编译期字面量，不能拼字符串，所以 `MenuItem` 的图标属性从 `glyph: string`
  改成 `icon: image`，在 `.slint` 构造点写 `icon: @image-url("icons/calculator.svg")`；
  折叠箭头这类二选一可以直接 `source: cond ? @image-url(a) : @image-url(b)`。
- `Image` 默认 `image-fit: fill`，非正方盒会把图标拉变形，所以图标盒一律取正方形（11~16px），
  需要留白/对齐时在外面套定宽 `Rectangle`。

换成 SVG 后 `colorize` 才生效，图标第一次能跟随主题与状态变色：置顶态图钉染 `Theme.accent`、
无边框关闭键 hover 转白、菜单置灰项用 `Theme.text-secondary`（emoji 时代 `Text` 的 `color` 对彩色
emoji 无效，只能靠底色高亮表示选中）。窗口图标当时仍是 `ui/icon.png`（位图），后来一并换成矢量，见 3.5。

复测（同一套抓图通路，`SLINT_BACKEND=software` 与默认各一遍）：模式菜单 19 项、设置页 4 处、
科学键盘 2 处折叠箭头、顶栏图钉/历史、置顶态关闭键，两种渲染器下全部可见、无空白；
software 与默认两张菜单图 md5 仍不同（`a3084539…` vs `5d5f0ed4…`），排除"其实跑的是同一个渲染器"。
`cargo test` 31 + 21 通过，`cargo check --target x86_64-pc-windows-msvc` 通过（SVG 随二进制内嵌，Windows 侧不额外带文件）。

## 3.4 置顶（无边框）模式的自绘圆角（2026-10-01）

Windows 有 DWM 的 `DWMWA_WINDOW_CORNER_PREFERENCE`，Linux/macOS 没有对应 API：GNOME 的合成器只给
带装饰的窗口加圆角，无边框窗口一律直角。所以非 Windows 走"透明底 + 圆角矩形"的自绘方案：

- `ui/main.slint`：新增 `in-out property <float> corner-radius: 0;` 和派生的 `css-corners: root.corner-radius > 0`。启用时 `Window { background: transparent }`，
  并在最底层放一块 `Rectangle { background: Theme.window; border-radius: corner-radius * 1px }` 上色。
- `src/main.rs`：`#[cfg(not(windows))] ui.set_corner_radius(12.0);`。Windows 保持 0，继续由 DWM 负责，
  不让两条圆角路径叠加；半径只有一处真值，`.slint` 里 `* 1px` 转成 length。

关键前提是"Slint 能不能要到带 alpha 的表面"。答案是可以：`i-slint-backend-winit` 的
`WinitWindowAdapter::window_attributes()` 本来就写着 `WindowAttributes::default().with_transparent(true)`，
X11 上 winit 因此挑 32bpp ARGB visual，femtovg / wgpu 也按 `window_attributes.transparent` 配置表面。
源码里只有 macOS 会在 background 变化时补调 `set_transparent`（`wants_transparent` 被
`#[cfg(target_os = "macos")]` 圈住），Linux 走的是"建窗即透明"这条更直接的路。

实测（`XGetImage` 读 32bpp 缓冲的 alpha 通道，半径设 12）：

| 状态 | 四角 alpha | 全透明像素 | 中心 alpha |
|---|---|---|---|
| 置顶，默认渲染器（GL/femtovg） | 0 | 92（另有一圈半透明过渡像素） | 255 |
| 置顶，`SLINT_BACKEND=software` | 0 | 184 | 255 |
| 普通（带标题栏） | 255 | 0（220400/220400 全不透明） | 255 |

轮廓与圆方程对照：y=0..7 行首个不透明像素实测 x = 7,5,4,3,2,1,1,0，与 r≈11 的圆弧一致
（r=12 的理论值是 8.6,6.2,4.7,3.5,2.6,1.9,1.3,0.9，Slint 的光栅化略紧一档），不是 45° 斜切。
右下角的 `=` 键会被这段圆弧切掉一角——与 Windows 上 DWM 遮罩裁内容的表现一致，属预期；
顶栏内容都在 4px 内边距 + 12px 半径的圆弧以内，没有被切到的。
切换 no-frame 会重建窗口，透明属性随新窗口一起重建，实测重建后四角仍是 alpha=0。

边界：只在有合成器时成立。裸 X11 且没有合成器（或不支持 per-pixel alpha 的桌面）时四角会露出黑底；
macOS 走同一开关，但本机无法实测。投影没做——要投影得再留一圈透明边距，会把内容整体内缩。

## 3.5 应用图标改为自绘矢量电子计算器（2026-10-01）

原来的 `ui/icon.png` 是一张彩色算盘图（当年 emoji 路线的产物，见 docs/踩坑记录.md 第 5 节），
和本项目"电子计算器复刻"的主题并不贴。现在它 `git mv` 到 `ui/icons/icon-abacus.png` 留作历史参考，
窗口图标换成 `ui/icons/app-calculator.svg`：

```slint
Window {
    icon: @image-url("icons/app-calculator.svg");   // ui/main.slint
}
```

设计（64×64 viewBox，全部用 rect/circle 手排，没有外部依赖）：机身 `50×60` 圆角矩形 `rx=10`，
深灰渐变 `#3a4353 → #1e242f`，外圈 12% 白描边（深色任务栏勾得出轮廓，浅色背景上几乎不可见）；
显示屏 `38×13 rx=4` 走玻璃渐变，里面四条 `3.4×7.4` 的位段加一个小数点表示"在算数"；
键盘按 4 列 × 4 行栅格算——列 x = 13 / 23.17 / 33.33 / 43.5（键宽 7.5），行 y = 26 / 34 / 42 / 50（键高 6），
键面 `rx=2.2`，末行左侧是跨两格的 0 键（`17.67` 宽），最右列四键用 `Theme.accent` 的 `#4cc2ff` 当运算符/等号。

复测（同一套 X11 抓图通路，从活的窗口读 `_NET_WM_ICON`）：属性 16392 字节、内含单张 64×64、
非透明像素 71.6%——与机身 `50×60` 在 64×64 画布里的占比（3000/4096 = 73.2%，再扣掉四个圆角）对得上，
说明 Slint 侧的 resvg 光栅化没有裁切也没画空。64 / 32 / 16 三档缩放拼图后仍可辨认为计算器；
同一档对照旧的算盘位图，16px 下珠子并成一团。SVG 随二进制内嵌，Windows 侧不需要额外带文件。

## 3.6 顶栏两个模式入口 + 普通模式底部圆角（2026-10-01）

**「标准/科学」文本改成模式菜单入口。** 原来只有 ≡ 那 26×26 能点开抽屉，旁边的模式名是纯 `Text`。
现在它和 ≡ 一样是一个 `Rectangle { TouchArea + hover 底色 + Text }` 单元（宽 30→34px，容纳 hover 高亮块），
`clicked => root.drawer = !root.drawer`，与原版一致：点 ≡ 或点模式名都开抽屉。

**普通模式补底部两角。** 3.4 只处理了置顶模式，普通（带标题栏）模式下顶部两角由窗口管理器圆掉，
底部两角仍是直角。Slint 的 `Rectangle` 只有统一的 `border-radius`，没有逐角半径，所以用"把圆弧挪出窗外"：
`corner-bg` 在普通模式取 `y = -radius`、`height = root.height + radius`，顶部那段圆弧整段落在窗口外被裁掉，
露出来的只有底部两角；置顶模式回到 `y = 0` 四角都圆。`css-corners` 的判据也从
`pinned && radius > 0` 放宽为 `radius > 0`。

## 3.7 点击级验证通道（Slint system-testing）

前面几节的界面实测都只能看静态抓图，因为**本机 XWayland 下合成输入点不到窗口**：
窗口折算到 X root 的坐标是负的（实测 `(-870,-385)`），`XWarpPointer` 和 XTest 的 MotionNotify
都被夹到 root 边界 `(0,0)`；`XSendEvent` 造的事件又被 winit 按 `send_event` 标志丢掉。
所以点击类改动原先只能记成"无法实测"。现在走 Slint 自己的测试通道：
`slint = { features = ["system-testing"] }` 加 `SLINT_EMIT_DEBUG_INFO=1` 构建，运行时设
`SLINT_TEST_SERVER=127.0.0.1:<port>`——**app 反向连到测试脚本监听的 TCP**，
帧格式是 4 字节大端长度 + protobuf（见 `slint_systest.proto`）。`RequestFindElementsById` 拿元素句柄，
`RequestElementClick` 派发真实点击；点击仍走正常命中测试，所以遮罩照吞不误（这正是想要的语义）。
元素 id 带组件前缀，`menu-ta` 要写成 `MainWindow::menu-ta`。

实测（`/tmp/systest.py`，脚本不入库）：

| 断言 | 结果 |
|---|---|
| ≡ 与「标准」都是可点热区 | `MainWindow::menu-ta` 26×28 @ (6,6)、`MainWindow::mode-ta` 34×28 @ (36,6)，各点一次都能开抽屉 |
| 普通模式只圆底部两角 | 顶部 6 行首个不透明像素 x 全为 0（直角），底部 6 行是 12,7,6,4,3,3（圆弧），全透明像素 46；科学型 380×620 下同样是 46/235600 |
| 置顶模式四角仍圆（属性改名回归） | 四角 alpha 均为 0、全透明 92 像素，顶部与底部边界同为 12,7,6,4,3,3 |
| 置顶↔普通来回切 | 两次 `pin-ta` 点击后窗口稳定 380×580，圆角状态跟着 `pinned` 正确翻转 |

验证完把 `Cargo.toml` 里的 `features = ["system-testing"]` 撤掉：它会把测试服务端编进产物，
Slint 自己也标注"不建议用于发布构建"。

## 3.8 角度单位收进「三角学」浮层第一行（2026-10-01）

DEG/RAD 原先是显示区左上角一块独立芯片，和它真正作用的三角函数键隔了整个键盘。现在删掉那块芯片，
把它做成三角学浮层的**第一行**：`row: 0; col: 0; colspan: 4` 的整宽行，左侧写「角度单位」、
右侧用强调色回显当前值（`度 (DEG)` / `弧度 (RAD)`），点一下在两者间切换并顺带收起浮层
（同其它非 sticky 选项的行为）。浮层因此从 `height: 106px` 长到 `156px`，
原来的 2×4 函数格整体下移一行（`row: i / 4 + 1`）。

滑层收起后当前单位仍看得见：单位挂在分组标题「三角学」后面（11px 次要色后缀），
所以点「函数」浮层时它也还在。

实测（`/tmp/systest.py`，走 3.7 的点击通道）：

| 断言 | 结果 |
|---|---|
| 新行是可点热区且占满浮层宽 | `ScientificKeypad::angle-ta` 尺寸 330×46 @ (11,246)，浮层宽 340 |
| 点一次真的换单位 | 点前浮层显示「度 (DEG)」，点后浮层收起且标题变成「三角学 RAD」，再开显示「弧度 (RAD)」，再点回「三角学 DEG」 |
| 三角学浮层 | 3 行：角度单位 / 2nd sin cos tan / hyp sec csc cot |
| 函数浮层不受影响 | 仍是 2×3（`\|x\|` floor ceil / rand dms deg），高度未变 |
| 标准型不受影响 | 窗口仍 380×580，显示区没有残留芯片 |

## 4. 关键设计细节

### 4.1 Slint ↔ Rust 接口

```slint
export component MainWindow inherits Window {
    in property <string> expression-text;   // 公式行（含光标式高亮预留）
    in property <string> result-text;       // 结果行
    in property <bool> is-error;            // 错误文案样式
    in property <bool> memory-busy;         // M 徽标
    in property <bool> history-open;
    in-out property <bool> input-focused;   // 键盘事件路由

    in property <[HistoryItem]> history;    // 结构体模型
    callback history-selected(int);
    callback clear-history();

    callback key(string);   // 统一按键码：数字/运算/一元/功能，同 slint-calculator 约定并扩展
}
```

按键码扩展：`"0".."9" "." "+ - * /" "= clear ce backspace percent negate"` + 一元 `"inv square sqrt"` + 记忆 `"ms m+ m- mr mc"`。

### 4.2 AppController 状态机（比 slint-calculator 的关键差异）

原版是**表达式模式**而非计算器栈模式：

- `Input`：正在输入的数字（允许直接追加 `. `）
- `Expression(Vec<Token>)`：完整表达式；`×÷` 后自动补 `×` 占位、`+−` 后删除尾部悬空 `+−`（原版行为）
- `=` → parser+eval → 结果态；再按数字开启新表达式，再按运算符在结果上继续
- 一元运算（`x² √x ⅟x ±`）作用于"当前输入的末尾数字"，即时求值不影响优先级结构
- Esc = C（全清）；Ctrl+H 切换历史侧栏；Backspace 只在输入态删字符

每次状态迁移后由 `sync_ui()` 统一写 3 个属性（expression/result/memory），避免 UI 逻辑散落。

### 4.3 引擎表驱动行为（测试即规格）

| 输入 | 期望 | 覆盖点 |
|---|---|---|
| `2+3*4=` | `14` | 优先级 |
| `2*3+4*5=` | `26` | 混合 |
| `0.1+0.2=` | `0.3` | Rational 精确 |
| `50+10%=` | `55` | 上下文百分号 |
| `2+=` | 等待第二项 | 悬空运算符 |
| `5/0=` | 错误"除数不能为零。" | 错误态 |
| `9+=` 后再按 `*` | 表达式变 `9*` | 替换尾运算符 |
| `2` `x²` `=` | `4`，公式 `sqr(2)` | 一元 |
| `1,234,567` | 显示分组、参与运算 | 格式化 |
| `1/3+1/6=` | `0.5` | 分数中间态 |

### 4.4 历史与记忆

- `HistoryItem { name: string /*公式*/, value: string /*结果*/ }`，Slint struct model；
  新记录插顶部，上限 100；点击某条把结果写回输入态
- 记忆为单个 `Rational`；`M+ / M-` 后徽标保持；`MC` 或"清空记忆"弹窗清除

## 5. 里程碑与实施步骤

| 里程碑 | 内容 | 验收 |
|---|---|---|
| **M1 引擎** | lexer/parser/value/eval/format 纯 Rust + `engine_tests.rs` 全绿 | `cargo test`（无 GUI） |
| **M2 最小 UI** | theme + 双行显示 + KeyGrid，接 `key()`，标准运算跑通 | `cargo run` 手测 4 则+优先级 |
| **M3 状态机完整版** | 悬空运算符、一元运算、CE/C、错误态、格式化显示 | 行为表 10 项手测通过 |
| **M4 键盘+记忆** | FocusScope 键盘映射、MemoryPanel、M 徽标 | 纯键盘可完成计算 |
| **M5 历史面板** | ListView 侧栏、展开/收起、点击回填、清空 | 历史记录可交互 |
| **M6 打磨** | hover/pressed 动画、窗口最小尺寸、模式菜单占位、README | 视觉对照原版截图逐区核对 |

每个里程碑独立可运行、可编译；M1 完成前 UI 不接线（防返工）。

## 6. 风险与对策

1. **Rational 溢出**：`999999999*999999999` 中间乘积超 i64 → 用 i128 中间量 + 超限降级 f64；行为与原版"科学计数法"一致
2. **Win11 Fluent 细节还原成本**（hover 渐变、Mica 背景）→ Slint 无法直接拿系统 Mica，用纯色近似 + 卡片阴影，视觉验收按"神似"标准
3. **键盘焦点路由**：Slint `FocusScope` 与控件内 LineEdit 无冲突（本 UI 无输入框），但需 `input-focused` 绑定 `width>0` 保证窗口级捕获
4. **原版行为细节多**（如 `√` 后自动补括号、`±` 对 `0` 的处理）→ 先以 `docs/复刻计划.md` 行为表为准，实现期逐项勾验收，不追求 100% 像素/行为复刻

## 7. 参考资料

- 原版源码（C++/WinRT）：https://github.com/Microsoft/calculator
  - 标准模式行为：`src/Calculator/Views/`、计算器状态机在 `src/CalculatorViewModel/`（ComparisonEngine 即表达式引擎，可对照其优先级实现）
  - UI 布局/尺寸：`src/Calculator/UI/StandardCalculator*`、XAML `Views/CalculatorStandardOperator.xaml`
- 官方文档：https://docs.slint.dev （Model/ListView：Reference → Elements → ListView；键盘：FocusScope）
- 本仓库既有经验：`slint_projects/slint-calculator/设计步骤.md`（踩坑记录 5 条同样适用）、`slint_projects/UI教程.md`
