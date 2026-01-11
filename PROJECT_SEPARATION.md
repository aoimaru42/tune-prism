# プロジェクト分離の提案 📁

## 現状の問題

現在、`tune-prism`は`music-rs`の中に配置されていますが、これには以下の問題があります：

1. **プロジェクトの独立性が失われる** - 2つの異なるプロジェクトが混在
2. **依存関係の管理が複雑** - `music-rs`と`tune-prism`で異なる依存関係
3. **ビルド時間の増加** - 不要な依存関係も一緒にビルドされる
4. **理解しにくい** - どちらのプロジェクトのコードか分かりにくい

## 提案: プロジェクトを完全に分離

### 推奨構造

```
~/Documents/app/
├── music-rs/              # シンプルな音楽生成アプリ
│   ├── src/
│   │   ├── main.rs
│   │   ├── app.rs
│   │   └── wave/
│   └── Cargo.toml
│
└── tune-prism/            # 楽器分離アプリ（独立したプロジェクト）
    ├── src/
    ├── src-tauri/
    └── package.json
```

## music-rsからtune-prismを使う方法

### 方法1: CLIツールとして実行（推奨）

`tune-prism`をビルドしてバイナリを生成し、`music-rs`からCLIツールとして実行します。

#### メリット
- ✅ 完全に独立したプロジェクト
- ✅ 依存関係が混在しない
- ✅ シンプルで理解しやすい
- ✅ ビルド時間が短縮される

#### 実装方法

1. **tune-prismを独立したCLIツールに変更**

```rust
// tune-prism/src-tauri/src/bin/split_audio.rs
use std::path::PathBuf;
use clap::Parser;

#[derive(Parser)]
struct Args {
    #[arg(short, long)]
    input: PathBuf,
    
    #[arg(short, long)]
    output: PathBuf,
    
    #[arg(short, long, default_value = "htdemucs")]
    model: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    
    // モデルを読み込む
    let model = Demucs::init(...)?;
    
    // 楽器分離を実行
    let stems = split_track(&model, &args.input, &args.output)?;
    
    println!("分離完了: {:?}", stems);
    Ok(())
}
```

2. **music-rsからCLIツールを呼び出す**

```rust
// music-rs/src/separation.rs
use std::process::Command;

pub fn split_audio(input: &Path, output: &Path) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let output = Command::new("tune-prism")
        .arg("--input")
        .arg(input)
        .arg("--output")
        .arg(output)
        .output()?;
    
    if output.status.success() {
        // 結果をパース
        Ok(parse_stems(&output.stdout)?)
    } else {
        Err(format!("エラー: {}", String::from_utf8_lossy(&output.stderr)).into())
    }
}
```

3. **Cargo.tomlに追加**

```toml
# tune-prism/src-tauri/Cargo.toml
[[bin]]
name = "tune-prism"
path = "src/bin/split_audio.rs"

[dependencies]
clap = { version = "4.0", features = ["derive"] }
```

### 方法2: ライブラリとして統合

`tune-prism`のコアロジックをライブラリとして抽出し、`music-rs`から直接使用します。

#### メリット
- ✅ 関数として直接呼び出せる
- ✅ エラーハンドリングが簡単
- ✅ 型安全性が保証される

#### デメリット
- ❌ 依存関係が複雑になる（PyTorch、libtorchなど）
- ❌ ビルド時間が長くなる
- ❌ プロジェクトが大きくなる

#### 実装方法

1. **tune-prismをライブラリとして公開**

```rust
// tune-prism/src-tauri/src/lib.rs
pub mod demucs;
pub mod audio;

pub use demucs::{split_track, Demucs};
pub use audio::{decode_file, encode_pcm_to_wav};
```

2. **music-rsから依存関係を追加**

```toml
# music-rs/Cargo.toml
[dependencies]
tune-prism = { path = "../tune-prism/src-tauri" }
```

3. **music-rsから使用**

```rust
// music-rs/src/separation.rs
use tune_prism::{split_track, Demucs};

pub fn separate_instruments(input: &Path, output: &Path) -> Result<Vec<PathBuf>> {
    let model = Demucs::init(...)?;
    split_track(&model, input, output)
}
```

### 方法3: プロセス間通信（IPC）

`tune-prism`を常駐プロセスとして起動し、IPC（Unix Domain Socket、Named Pipeなど）で通信します。

#### メリット
- ✅ モデルを一度だけ読み込む（高速）
- ✅ プロセスを独立させられる

#### デメリット
- ❌ 実装が複雑
- ❌ デバッグが難しい
- ❌ この規模のプロジェクトには過剰

## 推奨: 方法1（CLIツールとして実行）

現在の状況では、**方法1（CLIツールとして実行）**が最もシンプルで実装しやすいです。

## 実装手順

### ステップ1: tune-prismを別ディレクトリに移動

```bash
# 現在のディレクトリ構造
cd ~/Documents/app/music-rs

# tune-prismを親ディレクトリに移動
mv tune-prism ../tune-prism

# 新しい構造
cd ~/Documents/app
ls
# music-rs/
# tune-prism/
```

### ステップ2: tune-prismをCLIツールに変更

```bash
cd ../tune-prism
# CLIツールのコードを追加
```

### ステップ3: music-rsから呼び出す

```rust
// music-rs/src/separation.rs
// CLIツールを呼び出す関数を実装
```

## エラーについて

現在のエラーは、まだOpenMP（libtorch）の問題が残っている可能性があります。プロジェクトを分離することで、デバッグもしやすくなります。

---

**提案: まずプロジェクトを分離して、その後CLIツールとして実装することをお勧めします。**

