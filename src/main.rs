slint::include_modules!();

use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use slint_ms_calculator::app::App;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

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
}

impl UiCtx {
    fn sync(&self, a: &App) {
        let Some(ui) = self.weak.upgrade() else { return };
        let big = a.big_line();
        ui.set_result_font(fit_font(&big));
        ui.set_result_text(big.into());
        ui.set_expression_text(a.small_line().into());
        ui.set_memory_busy(a.memory_busy());
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

    let ctx = UiCtx { weak: ui.as_weak(), hist, seq: Rc::new(Cell::new(0)) };

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

    ctx.sync(&app.borrow());
    ui.run().unwrap();
}
