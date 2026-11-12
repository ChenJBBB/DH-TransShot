use crate::vocabulary::{Entry, SaveResult};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

fn directory(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .document_dir()
        .map(|p| p.join("DH-TransShot").join("生词本"))
        .map_err(|e| format!("无法定位文档文件夹：{e}"))
}

#[tauri::command]
pub async fn save_vocabulary(app: AppHandle, entry: Entry) -> Result<SaveResult, String> {
    let path = directory(&app)?;
    log::info!(
        "[Vocabulary] 收藏请求, 字符数={}",
        entry.text.chars().count()
    );
    tauri::async_runtime::spawn_blocking(move || crate::vocabulary::save(&path, &entry))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("收藏失败：{e}"))
}

#[tauri::command]
pub async fn get_vocabulary_directory(app: AppHandle) -> Result<String, String> {
    Ok(directory(&app)?.to_string_lossy().into_owned())
}

#[tauri::command]
pub async fn open_vocabulary_directory(app: AppHandle) -> Result<(), String> {
    let path = directory(&app)?;
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
        #[cfg(target_os = "windows")]
        let program = "explorer.exe";
        #[cfg(target_os = "macos")]
        let program = "open";
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        let program = "xdg-open";
        std::process::Command::new(program)
            .arg(path)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}
