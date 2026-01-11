// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{error::Error, fs, io::{self, Write}};
use tokio::sync::Mutex;
use tracing::Level;
use tracing_subscriber::fmt::format::FmtSpan;
use tauri::Manager;

use stem_split::{
    data::AppDb,
    demucs::{self, get_available_device, LazyModelLoader},
    routes::{
        project::{
            __cmd__create_project, __cmd__get_all_projects, create_project, get_all_projects,
        },
        split::{
            __cmd__split_stems, __cmd__split_vocal_instrumental_stems, __cmd__create_stems_zip,
            split_stems, split_vocal_instrumental_stems, create_stems_zip,
        },
    },
    util::get_base_directory,
};


#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // 最初に標準出力を確実にフラッシュ
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
    
    // 標準出力と標準エラー出力の両方に出力（ターミナルに確実に表示されるように）
    eprintln!("[main] Starting application...");
    println!("[main] Starting application...");
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
    
    // デバッグ用にロガーを有効化
    setup_global_subscriber();

    // OpenMPのスレッド数を制限して、複数のOpenMPライブラリ間の競合を防止
    // libtorchが使用するOpenMPの設定
    std::env::set_var("OMP_NUM_THREADS", "1");
    std::env::set_var("MKL_NUM_THREADS", "1");
    std::env::set_var("NUMEXPR_NUM_THREADS", "1");
    
    // 動的ライブラリの検索パスからHomebrewのlibompを除外
    // DYLD_LIBRARY_PATHを設定しないことで、libtorchに含まれるOpenMPのみを使用

    println!("[main] Creating project_data directory...");
    eprintln!("[main] Creating project_data directory...");
    let base_dir = get_base_directory();
    println!("[main] Base directory: {:?}", base_dir);
    eprintln!("[main] Base directory: {:?}", base_dir);
    std::io::stdout().flush().ok();
    std::io::stderr().flush().ok();
    
    fs::create_dir_all(base_dir.join("project_data"))
        .expect("Unable to ensure base_directory exists");

    eprintln!("[main] Initializing Tauri Builder...");
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_drag::init())
        .setup(|app| {
            println!("[setup] Running setup...");
            eprintln!("[setup] Running setup...");
            io::stdout().flush().ok();
            io::stderr().flush().ok();
            
            println!("[setup] App handle available");
            eprintln!("[setup] App handle available");
            io::stdout().flush().ok();
            
            // モデルファイルのパスを解決
            eprintln!("[setup] Resolving models.json...");
            let models_path = app
                .path_resolver()
                .resolve_resource("models/models.json")
                .ok_or_else(|| {
                    let error_msg = "failed to resolve models/models.json resource";
                    eprintln!("[setup] Error: {}", error_msg);
                    io::Error::new(io::ErrorKind::NotFound, error_msg)
                })?;

            eprintln!("[setup] Loading models from: {:?}", models_path);
            let models = demucs::models(&models_path).map_err(|e| {
                eprintln!("[setup] Error loading models: {:?}", e);
                io::Error::new(io::ErrorKind::Other, format!("Failed to load models: {:?}", e))
            })?;

            // htdemucs_6sモデルを優先的に使用（存在する場合）、なければhtdemucsを使用
            eprintln!("[setup] Finding model (htdemucs_6s or htdemucs)...");
            
            // まず、htdemucs_6sモデルファイルが存在するか確認
            let htdemucs_6s_path = app.path_resolver().resolve_resource("models/htdemucs_6s.pt");
            let (model_info, model_path) = if let Some(path) = htdemucs_6s_path {
                // ファイルが実際に存在するか確認
                if path.exists() {
                    // htdemucs_6s.ptが存在する場合
                    eprintln!("[setup] htdemucs_6s.pt found, using htdemucs_6s model");
                    let info = demucs::find_model(models.clone(), "htdemucs_6s")
                        .ok_or_else(|| {
                            let error_msg = "htdemucs_6s model config not found in models.json";
                            eprintln!("[setup] Error: {}", error_msg);
                            io::Error::new(io::ErrorKind::NotFound, error_msg)
                        })?;
                    (info, path)
                } else {
                    // パスは解決できたが、ファイルが存在しない場合
                    eprintln!("[setup] htdemucs_6s.pt path resolved but file does not exist, using htdemucs model");
                    let info = demucs::find_model(models, "htdemucs")
                        .ok_or_else(|| {
                            let error_msg = "model htdemucs is not available";
                            eprintln!("[setup] Error: {}", error_msg);
                            io::Error::new(io::ErrorKind::NotFound, error_msg)
                        })?;
                    let path = app
                        .path_resolver()
                        .resolve_resource("models/htdemucs.pt")
                        .ok_or_else(|| {
                            let error_msg = "failed to resolve models/htdemucs.pt resource";
                            eprintln!("[setup] Error: {}", error_msg);
                            io::Error::new(io::ErrorKind::NotFound, error_msg)
                        })?;
                    (info, path)
                }
            } else {
                // htdemucs_6s.ptが存在しない場合、htdemucsを使用
                eprintln!("[setup] htdemucs_6s.pt not found, using htdemucs model");
                let info = demucs::find_model(models, "htdemucs")
                    .ok_or_else(|| {
                        let error_msg = "model htdemucs is not available";
                        eprintln!("[setup] Error: {}", error_msg);
                        io::Error::new(io::ErrorKind::NotFound, error_msg)
                    })?;
                let path = app
                    .path_resolver()
                    .resolve_resource("models/htdemucs.pt")
                    .ok_or_else(|| {
                        let error_msg = "failed to resolve models/htdemucs.pt resource";
                        eprintln!("[setup] Error: {}", error_msg);
                        io::Error::new(io::ErrorKind::NotFound, error_msg)
                    })?;
                (info, path)
            };

            eprintln!("[setup] Using model: {}", model_info.name);
            eprintln!("[setup] Model file path: {:?}", model_path);

            eprintln!("[setup] Getting available device...");
            let device = get_available_device();
            eprintln!("[setup] Using device: {:?}", device);

            // モデルを遅延ロードするように設定（起動時はロードしない）
            eprintln!("[setup] Setting up lazy model loader (model will be loaded on demand)");
            let model_loader = LazyModelLoader::new(model_info, model_path, device);
            app.manage(Mutex::from(model_loader));
            eprintln!("[setup] Setup completed successfully (model not loaded yet to save memory)");
            Ok(())
        })
        .manage(Mutex::from(AppDb::new(get_base_directory().join("db"))))
        .invoke_handler(tauri::generate_handler![
            create_project,
            get_all_projects,
            split_stems,
            split_vocal_instrumental_stems,
            create_stems_zip,
        ]);
    
    println!("[main] About to run Tauri application...");
    eprintln!("[main] About to run Tauri application...");
    std::io::stdout().flush().ok();
    std::io::stderr().flush().ok();
    
    println!("[main] Generating Tauri context...");
    eprintln!("[main] Generating Tauri context...");
    std::io::stdout().flush().ok();
    
    let context = tauri::generate_context!();
    println!("[main] Context generated successfully");
    eprintln!("[main] Context generated successfully");
    std::io::stdout().flush().ok();
    
    println!("[main] Running Tauri application with context...");
    eprintln!("[main] Running Tauri application with context...");
    std::io::stdout().flush().ok();
    std::io::stderr().flush().ok();
    
    builder.run(context)
        .map_err(|e| {
            println!("[main] Error running Tauri application: {:?}", e);
            eprintln!("[main] Error running Tauri application: {:?}", e);
            std::io::stdout().flush().ok();
            std::io::stderr().flush().ok();
            e
        })?;

    println!("[main] Application exited successfully");
    eprintln!("[main] Application exited successfully");
    Ok(())
}

fn setup_global_subscriber() {
    // デバッグモードでのみロガーを初期化
    if cfg!(debug_assertions) {
        // 標準エラー出力に出力（Tauriのコンソールに表示される）
        tracing_subscriber::fmt()
            .with_writer(std::io::stderr)
            .with_span_events(FmtSpan::ENTER | FmtSpan::CLOSE)
            .with_max_level(Level::DEBUG)
            .init();
        eprintln!("[logger] Tracing subscriber initialized");
    }
}
