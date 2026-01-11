use snafu::ResultExt;
use tokio::sync::Mutex;
use std::fs::File;
use std::path::PathBuf as StdPathBuf;

use serde::{self, Deserialize, Serialize};
use tauri::State;

use crate::{
    data::AppDb,
    demucs::{split_track, split_vocal_instrumental, LazyModelLoader},
    routes::StemSplitSnafu,
    util::get_base_directory,
};

use super::{Error, Result};

#[derive(Serialize, Deserialize)]
#[serde(tag = "status")]
pub enum SplitStemsResponse {
    #[serde(alias = "success")]
    Success { stems: Vec<String> },
}

#[tauri::command]
#[tracing::instrument(skip(app_db_mutex, model_loader))]
pub async fn split_stems(
    project_id: &str,
    app_db_mutex: State<'_, Mutex<AppDb>>,
    model_loader: State<'_, Mutex<LazyModelLoader>>,
) -> Result<SplitStemsResponse> {
    let project_dir = get_base_directory().join("project_data").join(project_id);

    let song_path = project_dir.join("main.mp3"); // We're dealing with just MP3 for now.

    // ファイルが存在するかチェック
    if !song_path.exists() {
        return Err(Error::UnexpectedError {
            message: format!(
                "Audio file not found: {}. Please upload the audio file first.",
                song_path.display()
            ),
            source: None,
        });
    }

    // モデルを遅延ロード（初回のみロード、2回目以降は再利用）
    let mut loader = model_loader.lock().await;
    let model = loader.get_or_load().map_err(|e| Error::UnexpectedError {
        message: format!("Failed to load model: {}", e),
        source: Some(Box::new(e)),
    })?;

    let stem_paths = split_track(model, &song_path, &project_dir).context(StemSplitSnafu)?;

    let stems = stem_paths
        .clone()
        .into_iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    let app_db = app_db_mutex.lock().await;

    app_db
        .add_stems_to_project(String::from(project_id), stem_paths)
        .map_or(Err(Error::StemSaveError), |_| {
            Ok(SplitStemsResponse::Success { stems })
        })
}

#[tauri::command]
#[tracing::instrument(skip(app_db_mutex, model_loader))]
pub async fn split_vocal_instrumental_stems(
    project_id: &str,
    app_db_mutex: State<'_, Mutex<AppDb>>,
    model_loader: State<'_, Mutex<LazyModelLoader>>,
) -> Result<SplitStemsResponse> {
    let project_dir = get_base_directory().join("project_data").join(project_id);

    let song_path = project_dir.join("main.mp3");

    if !song_path.exists() {
        return Err(Error::UnexpectedError {
            message: format!(
                "Audio file not found: {}. Please upload the audio file first.",
                song_path.display()
            ),
            source: None,
        });
    }

    // モデルを遅延ロード（初回のみロード、2回目以降は再利用）
    let mut loader = model_loader.lock().await;
    let model = loader.get_or_load().map_err(|e| Error::UnexpectedError {
        message: format!("Failed to load model: {}", e),
        source: Some(Box::new(e)),
    })?;

    let stem_paths = split_vocal_instrumental(model, &song_path, &project_dir).context(StemSplitSnafu)?;

    let stems = stem_paths
        .clone()
        .into_iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    let app_db = app_db_mutex.lock().await;

    app_db
        .add_stems_to_project(String::from(project_id), stem_paths)
        .map_or(Err(Error::StemSaveError), |_| {
            Ok(SplitStemsResponse::Success { stems })
        })
}

#[tauri::command]
pub async fn create_stems_zip(
    _project_id: &str,
    stem_paths: Vec<String>,
    output_path: &str,
) -> std::result::Result<(), String> {
    eprintln!("[create_stems_zip] Creating ZIP file, output path: {}", output_path);
    eprintln!("[create_stems_zip] Stem paths: {:?}", stem_paths);
    
    use zip::write::{FileOptions, ZipWriter};
    use zip::CompressionMethod;
    use std::io::{BufWriter, Write};
    
    // ZIPファイルを作成
    let file = File::create(output_path)
        .map_err(|e| format!("Failed to create ZIP file: {}", e))?;
    let mut zip = ZipWriter::new(BufWriter::new(file));
    
    let options = FileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o755);
    
    // 各stemファイルをZIPに追加
    for stem_path in stem_paths {
        let stem_path_buf = StdPathBuf::from(&stem_path);
        
        // ファイル名を取得（パスから）
        let file_name = stem_path_buf
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| format!("Invalid file name: {}", stem_path))?;
        
        eprintln!("[create_stems_zip] Adding file to ZIP: {} (from: {})", file_name, stem_path);
        
        // ファイルを読み込む
        let file_data = std::fs::read(&stem_path)
            .map_err(|e| format!("Failed to read file {}: {}", stem_path, e))?;
        
        // ZIPに追加
        zip.start_file(file_name, options)
            .map_err(|e| format!("Failed to add file to ZIP: {}", e))?;
        zip.write_all(&file_data)
            .map_err(|e| format!("Failed to write file to ZIP: {}", e))?;
    }
    
    // ZIPファイルを完了
    zip.finish()
        .map_err(|e| format!("Failed to finish ZIP file: {}", e))?;
    
    eprintln!("[create_stems_zip] ZIP file created successfully: {}", output_path);
    
    Ok(())
}
