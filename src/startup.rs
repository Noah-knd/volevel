#[cfg(windows)]
mod imp {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
    use windows::Win32::System::Registry::{
        RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
        HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE, REG_SZ,
    };

    const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
    const VALUE_NAME: &str = "Volevel";

    fn to_wide(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(Some(0)).collect()
    }

    pub fn is_enabled() -> bool {
        let subkey = to_wide(RUN_KEY);
        let name = to_wide(VALUE_NAME);
        let mut hkey = HKEY::default();
        unsafe {
            if RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(subkey.as_ptr()),
                0,
                KEY_READ,
                &mut hkey,
            ) != ERROR_SUCCESS
            {
                return false;
            }
            let status = RegQueryValueExW(hkey, PCWSTR(name.as_ptr()), None, None, None, None);
            let _ = RegCloseKey(hkey);
            status == ERROR_SUCCESS
        }
    }

    pub fn set_enabled(enable: bool) -> Result<(), String> {
        let subkey = to_wide(RUN_KEY);
        let name = to_wide(VALUE_NAME);
        let mut hkey = HKEY::default();
        unsafe {
            let status = RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(subkey.as_ptr()),
                0,
                KEY_SET_VALUE,
                &mut hkey,
            );
            if status != ERROR_SUCCESS {
                return Err(format!("RegOpenKeyExW failed: {}", status.0));
            }
            let result = if enable {
                let exe = std::env::current_exe().map_err(|e| e.to_string())?;
                let cmd = format!("\"{}\" --minimized", exe.display());
                let wide = to_wide(&cmd);
                let bytes = std::slice::from_raw_parts(wide.as_ptr() as *const u8, wide.len() * 2);
                let res = RegSetValueExW(hkey, PCWSTR(name.as_ptr()), 0, REG_SZ, Some(bytes));
                if res == ERROR_SUCCESS {
                    Ok(())
                } else {
                    Err(format!("RegSetValueExW failed: {}", res.0))
                }
            } else {
                let res = RegDeleteValueW(hkey, PCWSTR(name.as_ptr()));
                if res == ERROR_SUCCESS || res == ERROR_FILE_NOT_FOUND {
                    Ok(())
                } else {
                    Err(format!("RegDeleteValueW failed: {}", res.0))
                }
            };
            let _ = RegCloseKey(hkey);
            result
        }
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn is_enabled() -> bool {
        false
    }

    pub fn set_enabled(_enable: bool) -> Result<(), String> {
        Ok(())
    }
}

pub use imp::{is_enabled, set_enabled};
