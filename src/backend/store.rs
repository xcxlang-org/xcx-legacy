use super::runtime;
use std::path::{Path, PathBuf};

fn confine(path_ptr: i64, path_len: i64) -> PathBuf {
    let text = runtime::read_text(path_ptr, path_len);
    let relative = Path::new(&text);
    if text.is_empty() {
        runtime::halt("store: the path is empty");
    }
    if relative.is_absolute() {
        runtime::halt(&format!(
            "store: `{text}` is an absolute path — paths are restricted to the project directory"
        ));
    }
    for component in relative.components() {
        if component == std::path::Component::ParentDir {
            runtime::halt(&format!(
                "store: `{text}` leaves the project directory — path traversal is not allowed"
            ));
        }
    }
    std::env::current_dir()
        .expect("the working directory is readable")
        .join(relative)
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_store_write(path_ptr: i64, path_len: i64, data_ptr: i64, data_len: i64) {
    let path = confine(path_ptr, path_len);
    let data = runtime::read_bytes(data_ptr, data_len);
    if let Some(directory) = path.parent() {
        if !directory.as_os_str().is_empty() && !directory.exists() {
            runtime::halt(&format!(
                "store: the directory `{}` does not exist",
                directory.display()
            ));
        }
    }
    if let Err(error) = std::fs::write(&path, data) {
        runtime::halt(&format!("store: cannot write `{}`: {error}", path.display()));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_store_read(path_ptr: i64, path_len: i64, out_len: *mut i64) -> i64 {
    let path = confine(path_ptr, path_len);
    match std::fs::read(&path) {
        Ok(bytes) => {
            unsafe { *out_len = bytes.len() as i64 };
            super::memory::allocate_bytes(bytes)
        }
        Err(error) => runtime::halt(&format!(
            "store: cannot read `{}`: the file does not exist or is unreadable ({error})",
            path.display()
        )),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_store_append(path_ptr: i64, path_len: i64, data_ptr: i64, data_len: i64) {
    let path = confine(path_ptr, path_len);
    let data = runtime::read_bytes(data_ptr, data_len);
    let result = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&path)
        .and_then(|mut file| std::io::Write::write_all(&mut file, &data));
    if let Err(error) = result {
        runtime::halt(&format!("store: cannot append to `{}`: {error}", path.display()));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_store_exists(path_ptr: i64, path_len: i64) -> i8 {
    let path = confine(path_ptr, path_len);
    i8::from(path.exists())
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_read_str(out_len: *mut i64) -> i64 {
    let mut line = String::new();
    match std::io::stdin().read_line(&mut line) {
        Ok(_) => {
            let line = line.trim_end_matches(['\n', '\r']);
            unsafe { *out_len = line.len() as i64 };
            super::memory::allocate_bytes(line.as_bytes().to_vec())
        }
        Err(error) => runtime::halt(&format!("input: cannot read a line: {error}")),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_read_int() -> i64 {
    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
    match line.trim().parse::<i64>() {
        Ok(value) => value,
        Err(_) => runtime::halt(&format!(
            "input: `{}` is not a valid int",
            line.trim()
        )),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_read_float() -> f64 {
    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
    match line.trim().parse::<f64>() {
        Ok(value) => value,
        Err(_) => runtime::halt(&format!(
            "input: `{}` is not a valid float",
            line.trim()
        )),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_read_bool() -> i8 {
    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
    match line.trim() {
        "true" => 1,
        "false" => 0,
        other => runtime::halt(&format!("input: `{other}` is not a valid boolean")),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_terminal_clean() {
    runtime::write_line("\u{1b}[2J\u{1b}[1;1H");
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_terminal_help() {
    runtime::write_line("available terminal commands:");
    runtime::write_line("  !clean  clears the terminal screen");
    runtime::write_line("  !exit   ends the XCX program");
    runtime::write_line("  !help   displays available commands");
}
