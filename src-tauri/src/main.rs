#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(target_os = "linux")]
    tiktok_now_lib::configure_linux_runtime_env();

    tiktok_now_lib::run();
}
