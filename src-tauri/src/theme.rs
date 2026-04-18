//! Thin Tauri-command wrappers around `librewin_common`'s theme/accent readers.

use librewin_common::{get_accent as lw_get_accent, get_theme as lw_get_theme};
use tauri::command;

#[command]
pub fn get_theme() -> String {
    lw_get_theme()
}

#[command]
pub fn get_accent() -> String {
    lw_get_accent()
}
