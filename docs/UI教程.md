# Slint UI 上手教程

> 面向第一次接触 Slint 的开发者。配合本目录下两个已完成项目学习：
> `slint-hello`（最小示例）和 `slint_projects/slint-calculator`（完整计算器）。

---

## 1. Slint 是什么

Slint 是一个**声明式 GUI 工具包**，用自有的 `.slint` DSL 描述界面，宿主逻辑用 Rust / C++ / JavaScript / Python 编写。特点：

- 界面与逻辑分离：`.slint` 管"长什么样"，宿主代码管"怎么算"
- 无 GC、渲染轻量，同一套 UI 可用于桌面（winit 后端）和嵌入式 MCU
- 官方组件库 `std-widgets.slint`：Button、LineEdit、ComboBox、ListView 等
- 成熟的编辑器实时预览工具链

## 2. 最小工程结构（本项目实际用法）

```
project/
├── Cargo.toml        # slint = "1.18"；build-dependencies: slint-build
├── build.rs          # slint_build::compile("ui.slint")
├── ui.slint          # 界面定义
└── src/main.rs       # slint::include_modules!() + 业务逻辑
```

三种把 `.slint` 编译进 Rust 的方式，按需求选一种：

| 方式 | 适用场景 |
|---|---|
| `build.rs` + `slint_build::compile()` + `include_modules!()` | 多文件工程、需要 IDE 跳转生成代码（推荐，计算器用的这种） |
| `slint::slint!{ ... }` 宏 | 单文件小工具，UI 直接内嵌在 Rust 里 |
| `slint-interpreter` 运行时加载 | UI 热更新、插件系统、不想重编译 |

## 3. 核心概念速览

### 3.1 元素树与组件

```slint
component MyCard inherits Rectangle {   // inherits 指定基元素
    in property <string> title;
    VerticalLayout {
        Text { text: root.title; }      // root = 当前组件实例
    }
}
```

- 元素写成嵌套块，属性用 `属性名: 值;`
- `root` / `parent` 引用父层；`id := 元素` 起名字后跨层引用（计算器里 `area := TouchArea` 就是这样）
- 只有 `export` 且继承 `Window` 的组件才会生成 Rust 代码；内部复用组件不要 export

### 3.2 属性与绑定（Slint 的灵魂）

属性不是"赋值一次"，而是**声明式绑定**——依赖变化自动重算：

```slint
in property <string> name: "World";
Text { text: "Hello " + name + "!"; }   // name 变，text 自动更新
```

- **方向修饰符**（本项目踩过的坑）：
  - `in`：宿主可写、组件内可读（Rust → UI）
  - `out`：组件内产出、宿主可读（UI → Rust）
  - `in-out`：双向
  - 不写修饰符 = 私有，宿主侧 get/set 会被编译为私有方法（E0624）
- **双向绑定**：`a.text <-> b.value`
- 常用类型：`length`（带单位 `12px`/`1mm`）、`color`（`#hex`/`rgba()`）、`string`、`int`、`float`、`bool`、`size`、`brush`

### 3.3 事件与回调

```slint
callback clicked();
area := TouchArea { clicked => { root.clicked(); } }   // 内置事件
```

- 内置事件挂在 `TouchArea`（指针）、`FocusScope`（键盘）、`ListView` 等元素上
- 宿主可订阅：Rust 里生成 `on_clicked(move || {...})`
- 事件块 `clicked => { ... }` 里可以写多语句，直接改属性，简单交互不用出 Rust

### 3.4 布局三件套

```slint
GridLayout {
    spacing: 8px;
    KeyButton { row: 4; col: 0; colspan: 2; }    // 0 键占两列
}
```

- `VerticalLayout` / `HorizontalLayout` / `GridLayout`，支持 `padding`、`spacing`、`alignment`
- 尺寸靠 `min-/max-/preferred-/width/height` 和 `stretch` 协商：
  - `vertical-stretch: 0` + 固定 `max-height` = 不被拉伸（计算器按键）
  - `vertical-stretch: 1` = 吃掉剩余空间（显示屏）
- 多行 HorizontalLayout 各算各的宽度会错位，**表格/键盘这类规整网格一律用 GridLayout**

### 3.5 常用元素

| 元素 | 用途 | 备注 |
|---|---|---|
| `Window` | 顶层窗口 | title、background、preferred-size |
| `Rectangle` | 圆角/描边色块 | border-radius、clip |
| `Text` | 文本 | 没有 padding！用 x/width 或套布局留边；没有 string.length 这类表达式 |
| `Image` | 图片 | `image-source: @image-url("a.png")` |
| `Path` | SVG 路径 | `@svg-paths` |
| `TouchArea` | 鼠标/触摸 | `pressed`、`clicked`、`entered` |
| `FocusScope` | 键盘事件 | `key-pressed(event)` |

组件库：`import { Button, LineEdit, Slider, ComboBox, ListView, GroupBox, TabWidget } from "std-widgets.slint";`

### 3.6 数据驱动：Model + Repeater

```slint
for item in model-data: MyCard {
    title: item.name;
}
ListView { for row in rows: Text { text: row; } }
```

宿主侧可用 `VecModel<T>` 动态增删，UI 自动同步。

### 3.7 动画与状态

```slint
states [
    active when root.pressed {
        transition: background.animate duration 120ms;
        inferred-property background: #ffbe55;
    }
]
```

或更简单：`animate background { duration: 120ms; }` + 属性绑定切换。

## 4. 宿主接线（Rust 侧模式）

```rust
slint::include_modules!();

let ui = Calculator::new().unwrap();
let state = Rc::new(RefCell::new(CalcState::new())); // 业务状态
let ui_weak = ui.as_weak();                          // 避免循环引用

ui.on_key(move |k: SharedString| {                   // callback key(string) -> on_key
    state.borrow_mut().on_key(&k.to_string());
    let Some(ui) = ui_weak.upgrade() else { return };
    ui.set_display_text(state.borrow().display.clone().into()); // String -> SharedString
});

ui.run().unwrap();
```

命名规则：`.slint` 里 kebab-case（`display-text`）生成 snake_case（`get_display_text`）。
常用 API：`get_xxx` / `set_xxx` / `on_callback` / `as_weak` / `run` / `show`。

## 5. 开发工具

| 工具 | 说明 |
|---|---|
| **VS Code 扩展 "Slint"** | 官方扩展，`.slint` 语法高亮 + 保存即热的实时预览窗格 |
| **slint-viewer** | 命令行预览：`cargo install slint-viewer --features renderer-svg`，然后 `slint-viewer ui.slint` |
| **SlintPad** | 浏览器在线编辑器，无需安装：https://slintpad.com （示例都在左侧列表里） |
| **gallery** | 源码仓库 `examples/gallery`：一个"控件全家桶"窗口，改一行 `.slint` 立即看效果 |

## 6. 推荐学习路径

1. **跑通 hello**（本仓库 `slint-hello`，30 分钟）：理解 build.rs、include_modules、in-out 属性、callback 四件事
2. **官方 Quickstart + Tutorial**：docs.slint.dev 上从 Rust quickstart 到 "Todo List 应用" 系列教程，逐步引入布局、控件、Model
3. **SlintPad 玩示例**：把 gallery、打印示例、计算器示例逐个打开改参数，建立"属性→效果"直觉
4. **抄一个官方 example**：`examples/` 目录下有 60+ 完整工程（滑块、音乐合成器、图片查看器、嵌入式 i.MX6…）
5. **啃 Language Reference**：elements、types、expressions、ESCAPES 写 `.slint` 时当字典查
6. **读 `slint-calculator` 的设计步骤.md**：本仓库如何把 UI/逻辑分离、如何做单元测试、5 条 Slint 特有坑

## 7. 本项目踩坑总结（省你半天）

1. `property <string> name: "初始值"` → 宿主 get/set 变私有（E0624）。宿主读写的属性要写 `in` / `in-out`
2. `Text` 没有 `left-padding`/`right-padding`；颜色没有 `.lighter()` 成员；字符串没有 `.length` 成员——表达式语法是受限的，别按 JS/QML 直觉写
3. 非 Window 组件 export 会警告且宿主拿不到，内部组件直接用 `component` 声明
4. `main.rs` 忘记 `slint::include_modules!()` → "cannot find type Xxx"
5. 多行 HorizontalLayout 网格列宽不齐 → 换 GridLayout + col/row/colspan
6. Rust 回调里改 UI 用 `Weak<计算器>::upgrade()`，不要 clone 强引用（会阻止析构）；String 传给 slint 属性用 `.into()` 转 `SharedString`

## 8. 参考资料

### 官方（首选）

- 文档主页（Guide / Tutorial / Reference 三栏）：https://docs.slint.dev
- Rust 快速上手：https://docs.slint.dev/latest/docs/slint/tutorial/quickstart/
- 语言参考（elements / types / expressions）：https://docs.slint.dev/latest/docs/slint/reference/overview/
- 多语言集成（C++/Node/Python/WASM）：https://docs.slint.dev/latest/docs/slint/language-integrations/
- 源码仓库（`examples/` 与 `tests/` 都是现成教材）：https://github.com/slint-ui/slint
- 在线编辑器 SlintPad：https://slintpad.com
- VS Code 市场搜 "Slint"（发布者 slint-ui）
- 官方论坛（提问比开 issue 友好）：https://github.com/slint-ui/slint/discussions
- changelog/新特性：https://slint.dev

### 社区中文

- [Slint UI开发终极指南：从入门到精通](https://m.blog.csdn.net/gitblog_00714/article/details/155927928)
- [超全Slint画廊示例：组件展示到交互设计](https://m.blog.csdn.net/gitblog_00771/article/details/151543088)
- [Slint布局系统深度探索](https://m.blog.csdn.net/gitblog_00571/article/details/155741130)
- [Slint多线程：并发处理与界面响应优化](https://blog.csdn.net/gitblog_00423/article/details/151236627)
- [Rust Slint实现白天黑夜切换开关源码分享](https://www.cnblogs.com/gccbuaa/p/19241450)
- [Slint：为现代应用打造的声明式GUI工具包](https://m.blog.csdn.net/gitblog_01018/article/details/142809714)
