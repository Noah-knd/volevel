#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod config;
mod engine;
mod i18n;
mod startup;
mod ui;
mod widgets;

use std::sync::Arc;

use config::AppConfig;
use ui::{decode_embedded_icon, VolevelApp};

#[cfg(windows)]
fn acquire_single_instance() -> Option<(windows::Win32::Foundation::HANDLE, usize)> {
    use windows::core::w;
    use windows::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS};
    use windows::Win32::System::Threading::{
        CreateEventW, CreateMutexW, OpenEventW, SetEvent, EVENT_MODIFY_STATE,
    };

    unsafe {
        let mutex = CreateMutexW(None, false, w!("Local\\VolevelSingleInstance")).ok()?;
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(mutex);
            if let Ok(ev) = OpenEventW(EVENT_MODIFY_STATE, false, w!("Local\\VolevelShowWindow"))
            {
                let _ = SetEvent(ev);
                let _ = CloseHandle(ev);
            }
            return None;
        }
        let show_event = CreateEventW(None, false, false, w!("Local\\VolevelShowWindow"))
            .map(|h| h.0 as usize)
            .unwrap_or(0);
        Some((mutex, show_event))
    }
}

fn main() -> eframe::Result<()> {
    #[cfg(windows)]
    let (_instance_guard, show_event) = match acquire_single_instance() {
        Some(pair) => pair,
        None => return Ok(()),
    };
    #[cfg(not(windows))]
    let show_event: usize = 0;

    let start_minimized = std::env::args().any(|a| a == "--minimized");
    let config = AppConfig::load();

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Volevel")
        .with_inner_size([780.0, 400.0])
        .with_min_inner_size([660.0, 320.0])
        .with_visible(!start_minimized);

    if let Some((rgba, width, height)) = decode_embedded_icon() {
        viewport = viewport.with_icon(Arc::new(egui::IconData {
            rgba,
            width,
            height,
        }));
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "Volevel",
        options,
        Box::new(move |cc| {
            Ok(Box::new(VolevelApp::new(
                cc,
                config,
                !start_minimized,
                show_event,
            )))
        }),
    )
}
