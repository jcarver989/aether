use crate::settings::WISP_HOME_ENV_MUTEX;
use std::path::Path;

pub fn with_wisp_home(path: &Path, f: impl FnOnce()) {
    let _guard = WISP_HOME_ENV_MUTEX
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let old = std::env::var_os("WISP_HOME");
    unsafe { std::env::set_var("WISP_HOME", path) };
    f();
    if let Some(value) = old {
        unsafe { std::env::set_var("WISP_HOME", value) };
    } else {
        unsafe { std::env::remove_var("WISP_HOME") };
    }
}

pub const CUSTOM_TMTHEME: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>name</key>
    <string>Custom</string>
    <key>settings</key>
    <array>
        <dict>
            <key>settings</key>
            <dict>
                <key>foreground</key>
                <string>#112233</string>
                <key>background</key>
                <string>#000000</string>
            </dict>
        </dict>
    </array>
</dict>
</plist>"#;
