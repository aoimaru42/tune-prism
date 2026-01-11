use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rustc-link-arg=-std=c++17");
    
    // libtorchが既にOpenMPを含んでいるため、追加でOpenMPをリンクしない
    // 複数のOpenMPライブラリがロードされると競合してセグメンテーションフォルトが発生する
    
    // LIBTORCH環境変数が設定されていない場合、プロジェクトルートのlibtorchディレクトリを使用
    let libtorch_path = if let Ok(path) = env::var("LIBTORCH") {
        PathBuf::from(path)
    } else {
        let manifest_dir = env::var("CARGO_MANIFEST_DIR")
            .expect("CARGO_MANIFEST_DIR should be set by Cargo");
        let mut libtorch_path = PathBuf::from(manifest_dir);
        libtorch_path.push("..");
        libtorch_path.push("libtorch");
        
        libtorch_path
            .canonicalize()
            .expect("Failed to canonicalize libtorch path")
    };
    
    let libtorch_path_str = libtorch_path.to_string_lossy().to_string();
    
    // 環境変数を設定（同じプロセス内で実行される子プロセスに継承される）
    env::set_var("LIBTORCH", &libtorch_path_str);
    println!("cargo:rustc-env=LIBTORCH={}", libtorch_path_str);
    println!("cargo:warning=Setting LIBTORCH to: {}", libtorch_path_str);
    
    // libtorchライブラリのパスをRPATHに追加（実行時にライブラリを見つけられるようにする）
    let libtorch_lib_path = libtorch_path.join("lib");
    if libtorch_lib_path.exists() {
        let libtorch_lib_path_str = libtorch_lib_path.to_string_lossy().to_string();
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", libtorch_lib_path_str);
        println!("cargo:warning=Adding RPATH: {}", libtorch_lib_path_str);
    }
    
    tauri_build::build()
}
