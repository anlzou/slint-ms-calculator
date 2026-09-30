slint::include_modules!();

use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use slint_ms_calculator::app::{App, CalcMode};
use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::rc::Rc;

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
fn force_rounded_corners() {
    let title: Vec<u16> = "计算器".encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let hwnd = FindWindowW(std::ptr::null(), title.as_ptr());
        if !hwnd.is_null() {
            let pref: u32 = 2;
            let _ = DwmSetWindowAttribute(hwnd, 33, &pref, 4);
        }
    }
}

/// 仿原版：按文本长度缩放结果行字号，避免截断（近似字宽 0.62em，可用宽约 330px）
fn fit_font(text: &str) -> f32 {
    let n = text.chars().count().max(1) as f32;
    (330.0 / (n * 0.62)).clamp(15.0, 34.0)
}

/// Slint 1.18 无系统深浅色 API，读注册表 AppsUseLightTheme（0=深色）
fn system_dark() -> bool {
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

/// 把 App 状态同步到 UI；历史模型只在版本号变化时整体重建
#[derive(Clone)]
struct UiCtx {
    weak: slint::Weak<MainWindow>,
    hist: Rc<VecModel<HistoryItem>>,
    seq: Rc<Cell<u64>>,
    sys_dark: Rc<Cell<bool>>,
    last_sci: Rc<Cell<bool>>,
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
    let hist = Rc::new(VecModel::<HistoryItem>::default());
    ui.set_history(ModelRc::from(hist.clone()));

    let ctx = UiCtx {
        weak: ui.as_weak(),
        hist,
        seq: Rc::new(Cell::new(0)),
        sys_dark: Rc::new(Cell::new(system_dark())),
        last_sci: Rc::new(Cell::new(false)),
    };

    {
        let app = app.clone();
        let ctx = ctx.clone();
        ui.on_key(move |k: SharedString| {
            app.borrow_mut().handle_key(&k.to_string());
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
            ctx.sync(&app.borrow());
        });
    }
    {
        let app = app.clone();
        let ctx = ctx.clone();
        ui.on_theme_mode_changed(move |_m: i32| {
            ctx.sys_dark.set(system_dark());
            ctx.sync(&app.borrow());
        });
    }
    ui.on_open_link(|url| {
        // Windows: start "" <url>；用 cmd 内建 start 打开默认浏览器
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", &url.to_string()])
            .spawn();
    });
    let close_weak = ui.as_weak();
    ui.on_close_requested(move || {
        // 先 hide 让窗口立即消失，再请求退出事件循环，确保进程正常结束
        if let Some(ui) = close_weak.upgrade() {
            let _ = ui.window().hide();
        }
        let _ = slint::quit_event_loop();
    });
    // 切换 no-frame 后窗口样式会重建：①圆角偏好要重新施加；②winit 不发 resize 事件，
    // 布局/渲染停留在旧状态（要手动拖边框才恢复）。用物理尺寸做两步微扰：
    // 先放大 2px、隔一拍再还原，强制两次真实的 WM_SIZE（同一帧内改了又改会被合并，无效）。
    let pin_weak = ui.as_weak();
    ui.on_pin_toggled(move || {
        force_rounded_corners();
        let weak = pin_weak.clone();
        slint::Timer::single_shot(std::time::Duration::from_millis(120), move || {
            force_rounded_corners();
            let Some(ui) = weak.upgrade() else { return };
            let win = ui.window();
            let s = win.size();
            win.set_size(slint::PhysicalSize::new(s.width + 2, s.height + 2));
            let weak2 = weak.clone();
            slint::Timer::single_shot(std::time::Duration::from_millis(120), move || {
                if let Some(ui) = weak2.upgrade() {
                    let win = ui.window();
                    let s = win.size();
                    win.set_size(slint::PhysicalSize::new(s.width - 2, s.height - 2));
                }
            });
        });
    });

    force_rounded_corners();
    ctx.sync(&app.borrow());
    ui.run().unwrap();
}
