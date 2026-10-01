//! 跨进程状态：落盘的记忆槽/历史/模式/角度，以及"把窗口从 Wayland 挪到 X11 重启"。
//!
//! 这两件事必须一起做：置顶（always-on-top）在原生 Wayland 下**做不到**——winit 0.30 的
//! Wayland 后端里 `set_window_level` 就是空函数（`platform_impl/linux/wayland/window/mod.rs`），
//! xdg_toplevel 协议本身也没有"保持在最前"这个状态，GNOME/mutter 也不给 D-Bus 接口。
//! 唯一的出路是让窗口走 X11 后端（winit 的选择逻辑只看 `WAYLAND_DISPLAY`/`WAYLAND_SOCKET`
//! 是否设置，没有 `WINIT_UNIX_BACKEND` 这类开关），也就是换个进程重开一次。
//! 换进程就会丢内存槽和历史，所以这里带一份很小的落盘状态，新进程起来先恢复。
//!
//! 状态文件是 tab 分隔的行式格式（不是 JSON，避免为几十行文本引入序列化依赖）：
//!
//! ```text
//! slint-ms-calc<TAB>v1
//! mode<TAB>scientific
//! angle<TAB>rad
//! theme<TAB>1                       # 0 浅 / 1 深 / 2 跟随系统
//! mem<TAB>E 12 5                      # 或 A <f64 Debug 形式>
//! hist<TAB>E 1 2<TAB>1 ÷ 2 =<TAB>0.5  # 值<TAB>表达式<TAB>展示结果，最新在前
//! ```
//!
//! 解析器对不认识的行一律跳过，格式将来变了也不会把用户挡在门外。

use std::io;
use std::path::PathBuf;

/// 置顶重启时传给新进程的环境变量：新实例直接以置顶模式起来，用户不用二次点击
pub const PIN_ON_START: &str = "SLINT_CALC_PIN_ON_START";

/// 状态文件路径。各平台按约定目录走，拿不到 HOME 就放弃持久化（返回 None，功能照常）。
pub fn state_path() -> Option<PathBuf> {
    #[cfg(windows)]
    let root = std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("APPDATA"))
        .map(PathBuf::from);
    #[cfg(target_os = "macos")]
    let root = std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"));
    #[cfg(not(any(windows, target_os = "macos")))]
    let root = std::env::var_os("XDG_STATE_HOME")
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))
        // 老环境没建过 ~/.local/state，退到同级的应用目录，避免依赖 XDG 目录存在
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".slint-ms-calculator")));

    root.map(|r| r.join("slint-ms-calculator").join("state.tsv"))
}

/// 读状态文本；文件不存在或读不动都返回 None（首次运行是正常路径）
pub fn load_text() -> Option<String> {
    std::fs::read_to_string(state_path()?).ok()
}

/// 原子写：先写同目录的 .tmp 再 rename，避免进程被杀时留下半份文件
pub fn save_text(text: &str) -> io::Result<()> {
    let path = state_path().ok_or_else(|| io::Error::other("找不到可写的状态目录"))?;
    let dir = path.parent().expect("state_path 总是带父目录");
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join("state.tsv.tmp");
    std::fs::write(&tmp, text)?;
    match std::fs::rename(&tmp, &path) {
        Ok(()) => Ok(()),
        // 极少数文件系统不允许覆盖改名（目标被别的进程占着）：退化成直接写
        Err(_) => std::fs::write(&path, text),
    }
}

/// 当前是否可能走原生 Wayland 表面（判据与 winit 的 linux 后端一致）
pub fn wayland_session() -> bool {
    let has = |k: &str| std::env::var_os(k).is_some_and(|v| !v.is_empty());
    has("WAYLAND_DISPLAY") || has("WAYLAND_SOCKET")
}

/// 能不能靠"重启到 X11"拿到真置顶：得是 Wayland 会话，且 X 侧确实有可连的显示
/// （GNOME/KDE 的 Wayland 会话默认带 XWayland，`DISPLAY` 非空；纯 Wayland 无 X 的环境
/// 重启后连窗口都开不出来，所以宁可不重启）。
pub fn can_restart_to_x11() -> bool {
    wayland_session() && std::env::var_os("DISPLAY").is_some_and(|v| !v.is_empty())
}

/// 以"没有 Wayland"的环境重启自己。调用方负责先落盘、再退出本进程。
pub fn restart_to_x11() -> io::Result<std::process::Child> {
    let mut cmd = std::process::Command::new(std::env::current_exe()?);
    cmd.env_remove("WAYLAND_DISPLAY").env_remove("WAYLAND_SOCKET");
    cmd.env(PIN_ON_START, "1");
    // 不继承 argv：本进程可能带着 -- 之类的调试参数，重启只要干净的默认启动
    cmd.spawn()
}

/// 本进程是否由"置顶重启"拉起（新实例据此直接进入置顶）
pub fn pin_on_start() -> bool {
    std::env::var_os(PIN_ON_START).is_some_and(|v| v == "1")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_path_is_under_app_dir() {
        // 只断言形状，不依赖真实 HOME：路径必须落在应用私有目录里、文件名固定
        let p = state_path().expect("测试环境总有 HOME 或 XDG_STATE_HOME");
        assert_eq!(p.file_name().unwrap(), "state.tsv");
        assert_eq!(p.parent().unwrap().file_name().unwrap(), "slint-ms-calculator");
    }
}
