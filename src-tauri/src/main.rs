//! 入口：发行版下不弹控制台窗口，其余交给库里的 [`decem_shell_lib::run`]。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    decem_shell_lib::run();
}
