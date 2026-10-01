slint::include_modules!();

use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use slint_ms_calculator::app::{App, CalcMode};
use slint_ms_calculator::persist;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// 平台相关部分：Win11 圆角、系统深浅色探测、默认浏览器打开链接。
/// Windows 之外不存在 dwmapi/user32、注册表和 cmd，这些符号必须按 cfg 隔离，
/// 否则 Linux/macOS 会在链接阶段报 unable to find library -ldwmapi / -luser32。
#[cfg(windows)]
mod platform {
    use std::ffi::c_void;

    #[link(name = "dwmapi")]
    unsafe extern "system" {
        fn DwmSetWindowAttribute(
            hwnd: *mut c_void,
            attr: u32,
            pv_attribute: *const u32,
            cb_attribute: u32,
        ) -> i32;
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        fn FindWindowW(class: *const u16, title: *const u16) -> *mut c_void;
    }

    /// Win11：DWMWA_WINDOW_CORNER_PREFERENCE(33) = DWMWCP_ROUND(2)。
    /// 无边框（置顶）模式下 DWM 可能给出直角，显式设置保持圆角。
    pub fn force_rounded_corners() {
        let title: Vec<u16> = "计算器".encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            let hwnd = FindWindowW(std::ptr::null(), title.as_ptr());
            if !hwnd.is_null() {
                let pref: u32 = 2;
                let _ = DwmSetWindowAttribute(hwnd, 33, &pref, 4);
            }
        }
    }

    /// Slint 1.18 无系统深浅色 API，读注册表 AppsUseLightTheme（0=深色）
    pub fn system_dark() -> bool {
        std::process::Command::new("reg")
            .args([
                "query",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
                "/v",
                "AppsUseLightTheme",
            ])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).contains("0x0"))
            .unwrap_or(false)
    }

    /// Windows: start "" <url>；用 cmd 内建 start 打开默认浏览器
    pub fn open_url(url: &str) {
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn();
    }
}

#[cfg(not(windows))]
mod platform {
    /// 非 Windows 没有 DWM 的圆角偏好可调：置顶模式四角、普通模式底部两角都由 .slint 自绘
    /// （见 ui/main.slint 的 corner-bg），这里无事可做。
    pub fn force_rounded_corners() {}

    /// GNOME/KDE 用 gsettings 的 color-scheme，其次看 gtk-theme 名；读不到就按浅色。
    #[cfg(target_os = "linux")]
    pub fn system_dark() -> bool {
        let gsettings = |key: &str| -> Option<String> {
            std::process::Command::new("gsettings")
                .args(["get", "org.gnome.desktop.interface", key])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).to_lowercase())
        };
        if let Some(scheme) = gsettings("color-scheme") {
            if scheme.contains("dark") {
                return true;
            }
            if scheme.contains("light") {
                return false;
            }
        }
        gsettings("gtk-theme")
            .map(|t| t.contains("dark"))
            .unwrap_or(false)
    }

    /// macOS：AppleInterfaceStyle 存在即为深色。
    #[cfg(target_os = "macos")]
    pub fn system_dark() -> bool {
        std::process::Command::new("defaults")
            .args(["read", "-g", "AppleInterfaceStyle"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).contains("Dark"))
            .unwrap_or(false)
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    pub fn system_dark() -> bool {
        false
    }

    /// Linux 用 xdg-open，macOS 用 open。
    pub fn open_url(url: &str) {
        #[cfg(target_os = "macos")]
        let mut cmd = std::process::Command::new("open");
        #[cfg(not(target_os = "macos"))]
        let mut cmd = std::process::Command::new("xdg-open");
        let _ = cmd.arg(url).spawn();
    }
}

/// 仿原版：按文本长度缩放结果行字号，避免截断（近似字宽 0.62em，可用宽约 330px）
fn fit_font(text: &str) -> f32 {
    let n = text.chars().count().max(1) as f32;
    (330.0 / (n * 0.62)).clamp(15.0, 34.0)
}

/// 把 App 状态同步到 UI；历史模型只在版本号变化时整体重建
#[derive(Clone)]
struct UiCtx {
    weak: slint::Weak<MainWindow>,
    hist: Rc<VecModel<HistoryItem>>,
    seq: Rc<Cell<u64>>,
    sys_dark: Rc<Cell<bool>>,
    last_sci: Rc<Cell<bool>>,
}

/// 记忆槽/历史/模式/角度落盘。每次按键后都写一份（几十行文本，成本可忽略），
/// 这样"置顶重启到 X11"和"下次打开计算器"都能拿到最新状态。
fn save_state(a: &App) {
    if let Err(err) = persist::save_text(&a.snapshot()) {
        eprintln!("状态落盘失败（不影响使用）: {err}");
    }
}

impl UiCtx {
    fn sync(&self, a: &App) {
        let Some(ui) = self.weak.upgrade() else { return };
        let big = a.big_line();
        ui.set_result_font(fit_font(&big));
        ui.set_result_text(big.into());
        ui.set_expression_text(a.small_line().into());
        ui.set_memory_busy(a.memory_busy());
        let sci = matches!(a.mode(), CalcMode::Scientific);
        ui.set_sci_mode(sci);
        // Slint 的 preferred-height 只在创建时生效：模式切换时手动改窗口尺寸
        if self.last_sci.get() != sci {
            self.last_sci.set(sci);
            let scale = ui.window().scale_factor();
            let h = if sci { 620.0 } else { 580.0 };
            ui.window().set_size(slint::PhysicalSize::new(
                (380.0 * scale) as u32,
                (h * scale) as u32,
            ));
        }
        ui.set_angle_label(a.angle_label().into());
        ui.set_second_active(a.second_active());
        let dark = match ui.get_theme_mode() {
            0 => false,
            1 => true,
            _ => self.sys_dark.get(),
        };
        ui.global::<Theme>().set_dark(dark);
        if self.seq.get() != a.history_seq() {
            self.seq.set(a.history_seq());
            self.hist.clear();
            self.hist.extend(a.history().iter().map(|h| HistoryItem {
                expression: h.expression.clone().into(),
                result: h.result.clone().into(),
            }));
        }
    }
}

fn main() {
    let ui = MainWindow::new().unwrap();
    let app = Rc::new(RefCell::new(App::new()));
    // 上次运行留下的记忆槽/历史/模式/角度/主题。读不到就是首次运行，走默认值。
    if let Some(text) = persist::load_text() {
        app.borrow_mut().restore(&text);
    }
    // 主题的归属在 App（要落盘的就是它），UI 的 theme-mode 只是显示：
    // 置顶在 Wayland 下靠重启实现，新实例若不从这里回填，用户显式选的浅色/深色会被
    // 打回"跟随系统"（实测：选浅色 → 点置顶 → 重启后窗口又变回深色）。
    ui.set_theme_mode(app.borrow().theme_mode());
    // 由"置顶重启到 X11"拉起的实例（见下面 on_pin_toggled）：直接以置顶模式起来，
    // 用户点一次置顶就该看到压在最前的窗口，不该还要再点一次。
    // 无边框可以立刻生效，但"恒在最前"要等窗口映射之后再放开闸门——
    // 映射前发的 _NET_WM_STATE 客户端消息会被 WM 丢掉（实测启动即置顶时 ABOVE 不出现）。
    if persist::pin_on_start() {
        ui.set_pinned(true);
        ui.set_level_pending(true);
        let weak = ui.as_weak();
        slint::Timer::single_shot(std::time::Duration::from_millis(300), move || {
            if let Some(ui) = weak.upgrade() {
                ui.set_level_pending(false);
            }
        });
    }
    let hist = Rc::new(VecModel::<HistoryItem>::default());
    ui.set_history(ModelRc::from(hist.clone()));

    // 自绘圆角半径：Windows 由 DWM 统一加圆角（platform::force_rounded_corners），传 0 走原路径；
    // Linux/macOS 的合成器不给无边框窗口加圆角、也不给普通模式客户区底部加圆角，只能自绘——
    // 把半径交给 .slint，窗口底色转透明、由圆角矩形上色（普通模式只切底部两角，见 main.slint 的 corner-bg）。
    #[cfg(not(windows))]
    ui.set_corner_radius(12.0);

    let ctx = UiCtx {
        weak: ui.as_weak(),
        hist,
        seq: Rc::new(Cell::new(0)),
        sys_dark: Rc::new(Cell::new(platform::system_dark())),
        last_sci: Rc::new(Cell::new(false)),
    };

    {
        let app = app.clone();
        let ctx = ctx.clone();
        ui.on_key(move |k: SharedString| {
            app.borrow_mut().handle_key(&k.to_string());
            save_state(&app.borrow());
            ctx.sync(&app.borrow());
        });
    }
    {
        let app = app.clone();
        let ctx = ctx.clone();
        ui.on_history_recall(move |i: i32| {
            app.borrow_mut().recall_history(i);
            ctx.sync(&app.borrow());
        });
    }
    {
        let app = app.clone();
        let ctx = ctx.clone();
        ui.on_history_clear(move || {
            app.borrow_mut().clear_history();
            save_state(&app.borrow());
            ctx.sync(&app.borrow());
        });
    }
    {
        let app = app.clone();
        let ctx = ctx.clone();
        ui.on_theme_mode_changed(move |m: i32| {
            // 存进 App 并落盘：主题得跟着"置顶重启"和下次启动走
            app.borrow_mut().set_theme_mode(m);
            save_state(&app.borrow());
            ctx.sys_dark.set(platform::system_dark());
            ctx.sync(&app.borrow());
        });
    }
    ui.on_open_link(|url| platform::open_url(&url.to_string()));
    let close_weak = ui.as_weak();
    ui.on_close_requested(move || {
        // 先 hide 让窗口立即消失，再请求退出事件循环，确保进程正常结束
        if let Some(ui) = close_weak.upgrade() {
            let _ = ui.window().hide();
        }
        let _ = slint::quit_event_loop();
    });
    // 切换 no-frame 后窗口样式会重建：①圆角偏好要重新施加；②winit 不发 resize 事件，
    // 布局/渲染停留在旧状态（要手动拖边框才恢复）；③Linux 实测重建会把客户区撑高
    // 37px（580→617，X11 去掉标题栏后把省下的空间算进客户区），必须显式还原目标尺寸。
    // 基准取回调触发瞬间的 size()：那时重建还没发生，量到的就是用户当前看到的真实尺寸
    //（实测入口 380x580，120ms 后已经是坏值）。还原仍走两步微扰：先 +2px、隔一拍再回到
    // 目标值，强制两次真实的 WM_SIZE（同一帧内改了又改会被合并，无效）。
    let pin_weak = ui.as_weak();
    let pin_app = app.clone();
    ui.on_pin_toggled(move || {
        // 原生 Wayland 下置顶做不到：winit 的 Wayland 后端把 set_window_level 实现成空函数，
        // xdg_toplevel 也没有"保持在最前"这个状态，GNOME/mutter 不提供对应接口。
        // 想真置顶只能让这个窗口走 X11 后端（winit 只看 WAYLAND_DISPLAY/WAYLAND_SOCKET 是否
        // 存在来选后端），也就是换个进程重开。先落盘（记忆/历史要跟着过去），
        // 再带着 SLINT_CALC_PIN_ON_START=1 重启，新窗口直接以置顶模式起来。
        // 只在"进入置顶"且本机确有 X 可连时这么做；失败就退回当前后端继续走原路径。
        if let Some(ui) = pin_weak.upgrade() {
            if ui.get_pinned() && persist::can_restart_to_x11() {
                save_state(&pin_app.borrow());
                match persist::restart_to_x11() {
                    Ok(_) => {
                        let _ = ui.window().hide();
                        let _ = slint::quit_event_loop();
                        return;
                    }
                    Err(err) => eprintln!("重启到 X11 失败，退回当前后端继续置顶: {err}"),
                }
            }
        }
        platform::force_rounded_corners();
        let target = pin_weak.upgrade().map(|u| u.window().size());
        let weak = pin_weak.clone();
        slint::Timer::single_shot(std::time::Duration::from_millis(120), move || {
            platform::force_rounded_corners();
            let Some(ui) = weak.upgrade() else { return };
            let win = ui.window();
            let cur = win.size();
            let (w, h) = match target {
                Some(t) if t.width > 0 && t.height > 0 => (t.width, t.height),
                _ => (cur.width, cur.height),
            };
            win.set_size(slint::PhysicalSize::new(w + 2, h + 2));
            let weak2 = weak.clone();
            slint::Timer::single_shot(std::time::Duration::from_millis(120), move || {
                if let Some(ui) = weak2.upgrade() {
                    ui.window().set_size(slint::PhysicalSize::new(w, h));
                }
            });
        });
    });

    platform::force_rounded_corners();
    ctx.sync(&app.borrow());
    ui.run().unwrap();
}
