//! Keyboard input handling for terminal

use alacritty_terminal::term::TermMode;
use gpui::KeyDownEvent;

/// Convert a key event to terminal input string
pub(crate) fn key_to_input(event: &KeyDownEvent, mode: &TermMode) -> String {
    let key = &event.keystroke.key;
    let modifiers = &event.keystroke;
    let app_cursor = mode.contains(TermMode::APP_CURSOR);

    if modifiers.modifiers.control {
        if key.len() == 1 {
            let c = key.chars().next().unwrap();
            if c.is_ascii_alphabetic() {
                let ctrl_char = (c.to_ascii_lowercase() as u8 - b'a' + 1) as char;
                return ctrl_char.to_string();
            }
        }
    }

    // Handle special keys first
    // Arrow keys and cursor keys change based on APP_CURSOR mode
    match key.as_str() {
        "enter" => return "\r".to_string(),
        "backspace" => {
            // Option+Backspace deletes word (send ESC + DEL)
            if modifiers.modifiers.alt {
                return "\x1b\x7f".to_string();
            }
            return "\x7f".to_string();
        }
        "tab" => {
            if modifiers.modifiers.shift {
                return "\x1b[Z".to_string();
            }
            return "\t".to_string();
        }
        "escape" => return "\x1b".to_string(),
        "up" => {
            if app_cursor {
                return "\x1bOA".to_string();
            }
            return "\x1b[A".to_string();
        }
        "down" => {
            if app_cursor {
                return "\x1bOB".to_string();
            }
            return "\x1b[B".to_string();
        }
        "right" => {
            // Option+Right: forward word (ESC + f)
            if modifiers.modifiers.alt {
                return "\x1bf".to_string();
            }
            if app_cursor {
                return "\x1bOC".to_string();
            }
            return "\x1b[C".to_string();
        }
        "left" => {
            // Option+Left: backward word (ESC + b)
            if modifiers.modifiers.alt {
                return "\x1bb".to_string();
            }
            if app_cursor {
                return "\x1bOD".to_string();
            }
            return "\x1b[D".to_string();
        }
        "home" => {
            if app_cursor {
                return "\x1bOH".to_string();
            }
            return "\x1b[H".to_string();
        }
        "end" => {
            if app_cursor {
                return "\x1bOF".to_string();
            }
            return "\x1b[F".to_string();
        }
        "pageup" => return "\x1b[5~".to_string(),
        "pagedown" => return "\x1b[6~".to_string(),
        "delete" => return "\x1b[3~".to_string(),
        "insert" => return "\x1b[2~".to_string(),
        "space" => return " ".to_string(),
        // Function keys
        "f1" => return "\x1bOP".to_string(),
        "f2" => return "\x1bOQ".to_string(),
        "f3" => return "\x1bOR".to_string(),
        "f4" => return "\x1bOS".to_string(),
        "f5" => return "\x1b[15~".to_string(),
        "f6" => return "\x1b[17~".to_string(),
        "f7" => return "\x1b[18~".to_string(),
        "f8" => return "\x1b[19~".to_string(),
        "f9" => return "\x1b[20~".to_string(),
        "f10" => return "\x1b[21~".to_string(),
        "f11" => return "\x1b[23~".to_string(),
        "f12" => return "\x1b[24~".to_string(),
        _ => {}
    }

    // Use key_char for actual typed character (handles shift for uppercase, etc.)
    if let Some(key_char) = &event.keystroke.key_char {
        if !key_char.is_empty() {
            return key_char.clone();
        }
    }

    // Fallback to key if it's a single character
    if key.len() == 1 {
        return key.clone();
    }

    String::new()
}
