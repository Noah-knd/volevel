use std::collections::HashMap;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

use crate::config::{AppConfig, AppRule, DuckingConfig};

#[derive(Debug, Clone)]
pub struct RgbaIcon {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct SessionView {
    pub key: String,
    pub display_name: String,
    pub base_volume: f32,
    pub effective_volume: f32,
    pub peak: f32,
    pub gain_reduction: f32,
    pub muted: bool,
    pub ducking_active: bool,
    pub rule: AppRule,
}

#[derive(Debug, Default)]
pub struct AudioSnapshot {
    pub sessions: Vec<SessionView>,
    pub icons: HashMap<String, RgbaIcon>,
}

pub enum AudioCommand {
    SetBaseVolume { key: String, volume: f32 },
    ToggleMute { key: String },
    UpdateRule { key: String, rule: AppRule },
    SetBypass(bool),
    UpdateDucking(DuckingConfig),
    SetLanguage(crate::config::Language),
    SetAutoStart(bool),
    ResetApp { key: String },
    ResetAll,
    SetHidden { key: String, hidden: bool },
    Shutdown,
}

pub struct AudioHandle {
    pub tx: mpsc::Sender<AudioCommand>,
    pub snapshot: Arc<Mutex<AudioSnapshot>>,
    join: Option<thread::JoinHandle<()>>,
}

impl AudioHandle {
    pub fn start(initial_config: AppConfig) -> Self {
        let (tx, rx) = mpsc::channel();
        let snapshot = Arc::new(Mutex::new(AudioSnapshot::default()));
        let snap_clone = Arc::clone(&snapshot);
        let join = thread::spawn(move || {
            platform::run_audio_loop(initial_config, rx, snap_clone);
        });
        Self {
            tx,
            snapshot,
            join: Some(join),
        }
    }

    pub fn shutdown(&mut self) {
        let _ = self.tx.send(AudioCommand::Shutdown);
        if let Some(handle) = self.join.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for AudioHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(windows)]
mod platform {
    use std::collections::HashMap;
    use std::ffi::OsString;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::path::Path;
    use std::sync::{mpsc, Arc, Mutex};
    use std::time::{Duration, Instant};

    use windows::core::{Interface, PCWSTR, PWSTR};
    use windows::Win32::Foundation::{CloseHandle, HWND, MAX_PATH, S_OK};
    use windows::Win32::Graphics::Gdi::{
        DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO,
        BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HGDIOBJ,
    };
    use windows::Win32::Media::Audio::Endpoints::IAudioMeterInformation;
    use windows::Win32::Media::Audio::{
        eConsole, eRender, AudioSessionStateActive, AudioSessionStateInactive, IMMDevice,
        IMMDeviceEnumerator, IAudioSessionControl2, IAudioSessionManager2, ISimpleAudioVolume,
        MMDeviceEnumerator,
    };
    use windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL,
        COINIT_MULTITHREADED,
    };
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};
    use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, ICONINFO};

    use super::{AudioCommand, AudioSnapshot, RgbaIcon, SessionView};
    use crate::config::AppConfig;
    use crate::engine::{ControlEngine, SessionInput};

    struct ComGuard;
    impl Drop for ComGuard {
        fn drop(&mut self) {
            unsafe { CoUninitialize() };
        }
    }

    struct RawSession {
        volume: ISimpleAudioVolume,
        meter: IAudioMeterInformation,
    }

    struct SessionGroup {
        key: String,
        display_name: String,
        exe_path: Option<String>,
        sessions: Vec<RawSession>,
    }

    struct EndpointState {
        device_id: String,
        manager: IAudioSessionManager2,
    }

    struct FailsafeRestore {
        config: AppConfig,
        groups: Vec<SessionGroup>,
    }

    impl FailsafeRestore {
        fn restore_all(&self) {
            for group in &self.groups {
                if let Some(rule) = self.config.apps.get(&group.key) {
                    let base = rule.base_volume.clamp(0.0, 1.0);
                    for s in &group.sessions {
                        unsafe {
                            let _ = s.volume.SetMasterVolume(base, std::ptr::null());
                        }
                    }
                }
            }
        }
    }

    impl Drop for FailsafeRestore {
        fn drop(&mut self) {
            self.restore_all();
        }
    }

    pub fn run_audio_loop(
        initial_config: AppConfig,
        rx: mpsc::Receiver<AudioCommand>,
        snapshot: Arc<Mutex<AudioSnapshot>>,
    ) {
        // Initialize multithreaded COM for Core Audio on this dedicated thread.
        let _com = unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            ComGuard
        };

        let enumerator: IMMDeviceEnumerator =
            match unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) } {
                Ok(e) => e,
                Err(_) => return,
            };

        let mut endpoint = open_default_endpoint(&enumerator);
        let mut engine = ControlEngine::new();
        let mut state = FailsafeRestore {
            config: initial_config,
            groups: Vec::new(),
        };
        let mut icon_cache: HashMap<String, Option<RgbaIcon>> = HashMap::new();

        let mut last_tick = Instant::now();
        let mut last_enum = Instant::now() - Duration::from_secs(1);
        let mut last_device_check = Instant::now() - Duration::from_secs(1);
        let mut dirty_since: Option<Instant> = None;

        loop {
            while let Ok(cmd) = rx.try_recv() {
                match cmd {
                    AudioCommand::SetBaseVolume { key, volume } => {
                        let vol = volume.clamp(0.0, 1.0);
                        let rule = state.config.rule_for(&key, vol);
                        rule.base_volume = vol;
                        engine.notify_manual_volume(&key, vol);
                        apply_volume_to_group(&state.groups, &key, vol);
                        dirty_since.get_or_insert_with(Instant::now);
                    }
                    AudioCommand::ToggleMute { key } => {
                        toggle_mute_group(&state.groups, &key);
                    }
                    AudioCommand::UpdateRule { key, rule } => {
                        let base = rule.base_volume.clamp(0.0, 1.0);
                        let turned_off = rule.mode == crate::config::ControlMode::Off;
                        state.config.apps.insert(key.clone(), rule);
                        if turned_off {
                            apply_volume_to_group(&state.groups, &key, base);
                            engine.notify_manual_volume(&key, base);
                        }
                        dirty_since.get_or_insert_with(Instant::now);
                    }
                    AudioCommand::SetBypass(bypass) => {
                        state.config.bypass = bypass;
                        engine.suppress_all_sync();
                        if bypass {
                            state.restore_all();
                        }
                        dirty_since.get_or_insert_with(Instant::now);
                    }
                    AudioCommand::UpdateDucking(ducking) => {
                        state.config.ducking = ducking;
                        dirty_since.get_or_insert_with(Instant::now);
                    }
                    AudioCommand::SetLanguage(lang) => {
                        state.config.language = lang;
                        dirty_since.get_or_insert_with(Instant::now);
                    }
                    AudioCommand::SetAutoStart(enabled) => {
                        state.config.auto_start = enabled;
                        dirty_since.get_or_insert_with(Instant::now);
                    }
                    AudioCommand::ResetApp { key } => {
                        let base = state
                            .config
                            .apps
                            .get(&key)
                            .map(|r| r.base_volume.clamp(0.0, 1.0))
                            .unwrap_or(1.0);
                        state.config.apps.insert(
                            key.clone(),
                            crate::config::AppRule {
                                base_volume: base,
                                ..Default::default()
                            },
                        );
                        apply_volume_to_group(&state.groups, &key, base);
                        engine.notify_manual_volume(&key, base);
                        dirty_since.get_or_insert_with(Instant::now);
                    }
                    AudioCommand::ResetAll => {
                        state.config.ducking = crate::config::DuckingConfig::default();
                        for rule in state.config.apps.values_mut() {
                            let base = rule.base_volume.clamp(0.0, 1.0);
                            *rule = crate::config::AppRule {
                                base_volume: base,
                                ..Default::default()
                            };
                        }
                        engine.suppress_all_sync();
                        state.restore_all();
                        dirty_since.get_or_insert_with(Instant::now);
                    }
                    AudioCommand::SetHidden { key, hidden } => {
                        if hidden {
                            state.config.hidden_apps.insert(key.clone());
                            if let Some(rule) = state.config.apps.get(&key) {
                                let base = rule.base_volume.clamp(0.0, 1.0);
                                apply_volume_to_group(&state.groups, &key, base);
                                engine.notify_manual_volume(&key, base);
                            }
                        } else {
                            state.config.hidden_apps.remove(&key);
                        }
                        dirty_since.get_or_insert_with(Instant::now);
                    }
                    AudioCommand::Shutdown => {
                        state.restore_all();
                        state.config.save();
                        return;
                    }
                }
            }

            if let Some(since) = dirty_since {
                if since.elapsed() >= Duration::from_millis(400) {
                    state.config.save();
                    dirty_since = None;
                }
            }

            if last_device_check.elapsed() >= Duration::from_millis(500) {
                last_device_check = Instant::now();
                let current_id = default_device_id(&enumerator);
                let changed = match (&endpoint, &current_id) {
                    (Some(ep), Some(id)) => ep.device_id != *id,
                    (None, Some(_)) => true,
                    (Some(_), None) => true,
                    (None, None) => false,
                };
                if changed {
                    state.restore_all();
                    endpoint = open_default_endpoint(&enumerator);
                    state.groups.clear();
                    last_enum = Instant::now() - Duration::from_secs(1);
                }
            }

            if last_enum.elapsed() >= Duration::from_millis(300) {
                last_enum = Instant::now();
                if let Some(ep) = &endpoint {
                    state.groups = enumerate_groups(&ep.manager);
                    for g in &state.groups {
                        if !icon_cache.contains_key(&g.key) {
                            let extracted = g.exe_path.as_deref().and_then(extract_exe_icon);
                            icon_cache.insert(g.key.clone(), extracted);
                        }
                    }
                } else {
                    state.groups.clear();
                }
            }

            let now = Instant::now();
            let dt_ms = (now - last_tick).as_secs_f32() * 1000.0;
            last_tick = now;

            let active_groups: Vec<&SessionGroup> = state
                .groups
                .iter()
                .filter(|g| !state.config.hidden_apps.contains(&g.key))
                .collect();

            let mut inputs = Vec::with_capacity(active_groups.len());
            for g in &active_groups {
                let mut peak = 0.0f32;
                let mut vol_sum = 0.0f32;
                let mut vol_cnt = 0usize;
                let mut all_muted = !g.sessions.is_empty();

                for s in &g.sessions {
                    unsafe {
                        if let Ok(p) = s.meter.GetPeakValue() {
                            peak = peak.max(p.clamp(0.0, 1.0));
                        }
                        if let Ok(v) = s.volume.GetMasterVolume() {
                            vol_sum += v.clamp(0.0, 1.0);
                            vol_cnt += 1;
                        }
                        if let Ok(m) = s.volume.GetMute() {
                            if !m.as_bool() {
                                all_muted = false;
                            }
                        }
                    }
                }

                let current_volume = if vol_cnt > 0 {
                    vol_sum / vol_cnt as f32
                } else {
                    1.0
                };

                inputs.push(SessionInput {
                    key: g.key.clone(),
                    peak,
                    current_volume,
                    muted: all_muted,
                });
            }

            let decisions = engine.step(&inputs, &mut state.config, dt_ms);

            for d in &decisions {
                if let Some(target_vol) = d.target_volume {
                    apply_volume_to_group(&state.groups, &d.key, target_vol);
                }
            }

            let mut views = Vec::with_capacity(active_groups.len());
            for (g, (inp, dec)) in active_groups
                .iter()
                .zip(inputs.iter().zip(decisions.iter()))
            {
                let rule = state
                    .config
                    .apps
                    .get(&g.key)
                    .cloned()
                    .unwrap_or_default();
                views.push(SessionView {
                    key: g.key.clone(),
                    display_name: g.display_name.clone(),
                    base_volume: rule.base_volume,
                    effective_volume: dec.effective_volume,
                    peak: dec.display_peak,
                    gain_reduction: dec.gain_reduction,
                    muted: inp.muted,
                    ducking_active: dec.ducking_active,
                    rule,
                });
            }

            if let Ok(mut snap) = snapshot.lock() {
                snap.sessions = views;
                for (k, v) in &icon_cache {
                    if let Some(icon) = v {
                        if !snap.icons.contains_key(k) {
                            snap.icons.insert(k.clone(), icon.clone());
                        }
                    }
                }
            }

            std::thread::sleep(Duration::from_millis(16));
        }
    }

    fn default_device_id(enumerator: &IMMDeviceEnumerator) -> Option<String> {
        unsafe {
            let dev: IMMDevice = enumerator.GetDefaultAudioEndpoint(eRender, eConsole).ok()?;
            let raw = dev.GetId().ok()?;
            let id = raw.to_string().ok();
            CoTaskMemFree(Some(raw.0 as *const _));
            id
        }
    }

    fn open_default_endpoint(enumerator: &IMMDeviceEnumerator) -> Option<EndpointState> {
        unsafe {
            let dev: IMMDevice = enumerator.GetDefaultAudioEndpoint(eRender, eConsole).ok()?;
            let raw = dev.GetId().ok()?;
            let device_id = raw.to_string().unwrap_or_default();
            CoTaskMemFree(Some(raw.0 as *const _));
            let manager: IAudioSessionManager2 = dev.Activate(CLSCTX_ALL, None).ok()?;
            Some(EndpointState { device_id, manager })
        }
    }

    fn enumerate_groups(manager: &IAudioSessionManager2) -> Vec<SessionGroup> {
        let mut by_key: HashMap<String, SessionGroup> = HashMap::new();
        unsafe {
            let Ok(session_enum) = manager.GetSessionEnumerator() else {
                return Vec::new();
            };
            let Ok(count) = session_enum.GetCount() else {
                return Vec::new();
            };

            for i in 0..count {
                let Ok(ctrl) = session_enum.GetSession(i) else {
                    continue;
                };
                let Ok(state) = ctrl.GetState() else {
                    continue;
                };
                if state != AudioSessionStateActive && state != AudioSessionStateInactive {
                    continue;
                }
                let Ok(ctrl2) = ctrl.cast::<IAudioSessionControl2>() else {
                    continue;
                };
                if ctrl2.IsSystemSoundsSession() == S_OK {
                    continue;
                }
                let Ok(pid) = ctrl2.GetProcessId() else {
                    continue;
                };
                if pid == 0 {
                    continue;
                }
                let Ok(volume) = ctrl.cast::<ISimpleAudioVolume>() else {
                    continue;
                };
                let Ok(meter) = ctrl.cast::<IAudioMeterInformation>() else {
                    continue;
                };

                let exe_path = query_process_exe_path(pid);
                let (key, display_name) = match &exe_path {
                    Some(p) => {
                        let file = Path::new(p)
                            .file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or("unknown.exe");
                        let key = file.to_ascii_lowercase();
                        let name = format_app_name(file);
                        (key, name)
                    }
                    None => (format!("pid:{pid}"), format!("App ({pid})")),
                };

                let entry = by_key.entry(key.clone()).or_insert_with(|| SessionGroup {
                    key,
                    display_name,
                    exe_path: exe_path.clone(),
                    sessions: Vec::new(),
                });
                entry.sessions.push(RawSession { volume, meter });
            }
        }

        let mut list: Vec<SessionGroup> = by_key.into_values().collect();
        list.sort_by(|a, b| a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase()));
        list
    }

    fn apply_volume_to_group(groups: &[SessionGroup], key: &str, volume: f32) {
        let vol = volume.clamp(0.0, 1.0);
        if let Some(g) = groups.iter().find(|g| g.key == key) {
            for s in &g.sessions {
                unsafe {
                    let _ = s.volume.SetMasterVolume(vol, std::ptr::null());
                }
            }
        }
    }

    fn toggle_mute_group(groups: &[SessionGroup], key: &str) {
        if let Some(g) = groups.iter().find(|g| g.key == key) {
            let currently_muted = !g.sessions.is_empty()
                && g.sessions.iter().all(|s| unsafe {
                    s.volume.GetMute().map(|b| b.as_bool()).unwrap_or(false)
                });
            let next_mute = !currently_muted;
            for s in &g.sessions {
                unsafe {
                    let _ = s.volume.SetMute(next_mute, std::ptr::null());
                }
            }
        }
    }

    fn query_process_exe_path(pid: u32) -> Option<String> {
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut buf = vec![0u16; MAX_PATH as usize * 4];
            let mut len = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(
                handle,
                PROCESS_NAME_WIN32,
                PWSTR(buf.as_mut_ptr()),
                &mut len,
            )
            .is_ok();
            let _ = CloseHandle(handle);
            if ok && len > 0 {
                let s = OsString::from_wide(&buf[..len as usize]);
                Some(s.to_string_lossy().into_owned())
            } else {
                None
            }
        }
    }

    fn format_app_name(filename: &str) -> String {
        let stem = filename
            .strip_suffix(".exe")
            .or_else(|| filename.strip_suffix(".EXE"))
            .unwrap_or(filename);
        if stem.chars().all(|c| !c.is_ascii_uppercase()) {
            let mut chars = stem.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => stem.to_string(),
            }
        } else {
            stem.to_string()
        }
    }

    fn extract_exe_icon(path: &str) -> Option<RgbaIcon> {
        // Extract shell icon HICON and read 32-bit BGRA pixels via GetDIBits, freeing all GDI handles.
        let wide: Vec<u16> = Path::new(path)
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        unsafe {
            let mut shfi = SHFILEINFOW::default();
            let res = SHGetFileInfoW(
                PCWSTR(wide.as_ptr()),
                FILE_FLAGS_AND_ATTRIBUTES(0),
                Some(&mut shfi),
                std::mem::size_of::<SHFILEINFOW>() as u32,
                SHGFI_ICON | SHGFI_LARGEICON,
            );
            if res == 0 || shfi.hIcon.is_invalid() {
                return None;
            }
            let hicon = shfi.hIcon;
            let mut info = ICONINFO::default();
            if GetIconInfo(hicon, &mut info).is_err() {
                let _ = DestroyIcon(hicon);
                return None;
            }

            let mut bmp = BITMAP::default();
            let got_obj = GetObjectW(
                HGDIOBJ(info.hbmColor.0),
                std::mem::size_of::<BITMAP>() as i32,
                Some(&mut bmp as *mut _ as *mut _),
            );

            let mut result = None;
            if got_obj > 0 && bmp.bmWidth > 0 && bmp.bmHeight > 0 {
                let w = bmp.bmWidth as usize;
                let h = bmp.bmHeight as usize;
                let mut bi = BITMAPINFO {
                    bmiHeader: BITMAPINFOHEADER {
                        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                        biWidth: bmp.bmWidth,
                        biHeight: -bmp.bmHeight,
                        biPlanes: 1,
                        biBitCount: 32,
                        biCompression: BI_RGB.0,
                        ..Default::default()
                    },
                    ..Default::default()
                };
                let mut pixels = vec![0u8; w * h * 4];
                let hdc = GetDC(HWND::default());
                if !hdc.is_invalid() {
                    let lines = GetDIBits(
                        hdc,
                        info.hbmColor,
                        0,
                        h as u32,
                        Some(pixels.as_mut_ptr() as *mut _),
                        &mut bi,
                        DIB_RGB_COLORS,
                    );
                    ReleaseDC(HWND::default(), hdc);
                    if lines > 0 {
                        let mut has_alpha = false;
                        for px in pixels.chunks_exact_mut(4) {
                            px.swap(0, 2);
                            if px[3] != 0 {
                                has_alpha = true;
                            }
                        }
                        if !has_alpha {
                            for px in pixels.chunks_exact_mut(4) {
                                if px[0] | px[1] | px[2] != 0 {
                                    px[3] = 255;
                                }
                            }
                        }
                        result = Some(RgbaIcon {
                            width: w,
                            height: h,
                            rgba: pixels,
                        });
                    }
                }
            }

            if !info.hbmColor.is_invalid() {
                let _ = DeleteObject(HGDIOBJ(info.hbmColor.0));
            }
            if !info.hbmMask.is_invalid() {
                let _ = DeleteObject(HGDIOBJ(info.hbmMask.0));
            }
            let _ = DestroyIcon(hicon);
            result
        }
    }
}

#[cfg(not(windows))]
mod platform {
    use std::sync::{mpsc, Arc, Mutex};
    use super::{AudioCommand, AudioSnapshot};
    use crate::config::AppConfig;

    pub fn run_audio_loop(
        _initial_config: AppConfig,
        rx: mpsc::Receiver<AudioCommand>,
        _snapshot: Arc<Mutex<AudioSnapshot>>,
    ) {
        while let Ok(cmd) = rx.recv() {
            if matches!(cmd, AudioCommand::Shutdown) {
                break;
            }
        }
    }
}
