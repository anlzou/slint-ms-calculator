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
Linux 圆角交给合成器（空实现）、深浅色读 `gsettings org.gnome.desktop.interface color-scheme`、链接用 `xdg-open`；
macOS 圆角同样空实现、深浅色读 `defaults read -g AppleInterfaceStyle`、链接用 `open`。
置顶切换后的 2px 尺寸微扰（winit 不发 resize 事件）三平台共用，故保留在通用代码里。

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
