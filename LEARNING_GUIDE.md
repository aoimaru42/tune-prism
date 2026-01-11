# Tune Prism 学習ガイド 🎓

`tune-prism`のコードを理解するための段階的な学習パスです。

## 📋 プロジェクトの全体像

### 技術スタック
- **Rust** - バックエンド（オーディオ処理、機械学習）
- **Tauri** - デスクトップアプリフレームワーク（Electronの軽量版）
- **React/TypeScript** - フロントエンド（UI）
- **PyTorch (libtorch)** - 機械学習モデル（HTDemucs）

### アプリの動作フロー
```
ユーザーがMP3をドラッグ&ドロップ
    ↓
フロントエンド（React）がファイルパスを取得
    ↓
Tauriコマンドでバックエンド（Rust）を呼び出し
    ↓
Rustがオーディオをデコード・リサンプリング
    ↓
PyTorchモデルで楽器分離（推論）
    ↓
各楽器ごとにWAVファイルとして保存
    ↓
フロントエンドに結果を返す
```

## 🎯 段階的な学習パス

### ステップ1: プロジェクト構造を理解する（1-2時間）

#### ディレクトリ構造
```
tune-prism/
├── src/                    # フロントエンド（React/TypeScript）
│   ├── App.tsx            # メインUIコンポーネント
│   ├── components/        # UIコンポーネント
│   └── functions/         # Tauriコマンドを呼び出す関数
│
└── src-tauri/             # バックエンド（Rust）
    ├── src/
    │   ├── main.rs        # エントリーポイント
    │   ├── lib.rs         # モジュール定義
    │   ├── routes/        # Tauriコマンド（APIエンドポイント）
    │   ├── demucs/        # 楽器分離のコアロジック
    │   └── data/          # データベース関連
    └── Cargo.toml         # Rustの依存関係
```

#### まず読むべきファイル（優先順位順）
1. **`src-tauri/src/main.rs`** - アプリのエントリーポイント
   - Tauriの初期化
   - モデルの読み込み
   - コマンドの登録

2. **`src-tauri/src/routes/mod.rs`** - APIの定義
   - エラーハンドリング
   - コマンドの構造

3. **`src-tauri/src/routes/split.rs`** - 楽器分離のエンドポイント
   - `split_stems`関数 - メインの処理フロー

### ステップ2: Tauriの基本を理解する（2-3時間）

Tauriは**Rustバックエンド**と**フロントエンド**を繋ぐフレームワークです。

#### 重要な概念
- **コマンド（Command）**: フロントエンドからバックエンドを呼び出す関数
  ```rust
  #[tauri::command]
  pub async fn split_stems(...) -> Result<...> {
      // 処理
  }
  ```

- **状態管理（State）**: アプリ全体で共有するデータ
  ```rust
  .manage(Mutex::from(AppDb::new(...)))  // データベース
  .manage(Demucs::init(...))              // 機械学習モデル
  ```

#### 学習リソース
- [Tauri公式ドキュメント](https://tauri.app/v1/guides/) - 日本語あり
- 特に重要: [Commands](https://tauri.app/v1/guides/features/command) セクション

#### 実践
1. `src-tauri/src/routes/split.rs`を読む
2. `src/functions/split.ts`を読む（フロントエンド側の呼び出し）
3. データの流れを追跡する

### ステップ3: オーディオ処理を理解する（3-4時間）

#### ファイル構造
```
demucs/
├── mod.rs       # メインロジック（split_track関数）
├── audio.rs     # オーディオのデコード/エンコード
├── model.rs     # PyTorchモデルのラッパー
└── error.rs     # エラー定義
```

#### 処理フロー
1. **デコード** (`audio.rs::decode_file`)
   - MP3/WAVなどのオーディオファイルを読み込む
   - PCM（生オーディオデータ）に変換
   - 使用ライブラリ: `symphonia`

2. **リサンプリング** (`audio.rs::resample`)
   - サンプルレートをモデルに合わせる（通常44100Hz）
   - 使用ライブラリ: `dasp`

3. **モデル推論** (`mod.rs::split_track`, `model.rs`)
   - PyTorchモデルで楽器分離
   - 使用ライブラリ: `tch` (PyTorch Rustバインディング)

4. **エンコード** (`audio.rs::encode_pcm_to_wav`)
   - PCMデータをWAVファイルに変換
   - 使用ライブラリ: `hound`

#### 学習リソース
- [symphoniaドキュメント](https://docs.rs/symphonia/) - オーディオデコード
- [houndドキュメント](https://docs.rs/hound/) - WAVファイル処理
- [tchドキュメント](https://docs.rs/tch/) - PyTorch Rustバインディング

#### 実践
1. `src-tauri/src/demucs/audio.rs`の`decode_file`を読む
2. 簡単なテストプログラムを作成
   ```rust
   // test_audio.rs
   use demucs::audio::decode_file;
   
   fn main() {
       let audio = decode_file("test.mp3").unwrap();
       println!("Channels: {}, Sample Rate: {}", 
                audio.nb_channels, audio.sample_rate);
   }
   ```

### ステップ4: 機械学習モデルを理解する（4-5時間）

#### PyTorchモデルの扱い方
- `model.rs` - PyTorchモデルのラッパー
- `Demucs`構造体 - モデルを保持・推論を実行

#### 重要な概念
- **テンソル**: 多次元配列（オーディオデータを表現）
- **デバイス**: CPU、GPU（MPS、CUDA）の選択
- **正規化**: モデルの入力データを標準化

#### モデルの処理フロー（`split_track`）
```rust
1. オーディオデータをテンソルに変換
2. 正規化（平均0、標準偏差1）
3. モデルに投入（推論）
4. 結果を正規化から戻す（デノーマライズ）
5. 各楽器ごとに分離して保存
```

#### 学習リソース
- [PyTorch公式ドキュメント](https://pytorch.org/docs/stable/index.html)
- [tchクレートのドキュメント](https://docs.rs/tch/)
- [HTDemucsの論文/リポジトリ](https://github.com/facebookresearch/demucs)

#### 実践
1. `src-tauri/src/demucs/model.rs`を読む
2. 小さなテンソル操作を試す
   ```rust
   use tch::Tensor;
   
   let x = Tensor::zeros(&[2, 3], tch::Kind::Float);
   println!("{:?}", x);
   ```

### ステップ5: エラーハンドリングを理解する（1-2時間）

#### 使用ライブラリ: `snafu`
- Rustのエラーハンドリングライブラリ
- エラーの種類を定義し、チェーン可能なエラーを作成

#### エラー定義
- `demucs/error.rs` - Demucs関連のエラー
- `routes/mod.rs` - API関連のエラー

#### 実践
1. `src-tauri/src/demucs/error.rs`を読む
2. `.context()`の使い方を理解する
3. `Result<T>`型の扱い方を理解する

## 📚 推奨学習リソース

### Rust基礎
1. **[The Rust Book（日本語版）](https://doc.rust-jp.rs/book-ja/)** - 必須
   - 特に重要な章: 所有権、エラーハンドリング、並行性

2. **[Rust by Example](https://doc.rust-lang.org/rust-by-example/)** - 実践的
   - コードを書いて試す

### Tauri
1. **[Tauri公式ドキュメント](https://tauri.app/v1/guides/)**
   - Getting Started
   - Frontend Integration
   - Commands

### オーディオ処理
1. **[Audio Signal Processing基礎](https://en.wikipedia.org/wiki/Audio_signal_processing)**
   - サンプルレート、チャンネル、PCMの概念

### 機械学習
1. **[PyTorch Tutorials](https://pytorch.org/tutorials/)**
   - テンソルの基礎
   - モデルの読み込みと推論

## 🛠️ 実践的な学習方法

### 1. コードを読む順番
```
1. main.rs → アプリの全体構造を理解
2. routes/split.rs → メインの処理フローを理解
3. demucs/mod.rs → 楽器分離のロジックを理解
4. demucs/audio.rs → オーディオ処理の詳細を理解
5. demucs/model.rs → 機械学習モデルの扱いを理解
```

### 2. 小さな変更から始める
- ログメッセージを追加
- エラーメッセージを改善
- コメントを追加

### 3. デバッグプリントを追加
```rust
println!("デバッグ: ここを通りました");
dbg!(&変数);  // 変数の内容を表示
```

### 4. テストプログラムを作成
```rust
// examples/simple_split.rs
fn main() {
    // シンプルな楽器分離のテスト
}
```

### 5. 公式ドキュメントを読みながら
- 分からない関数や型が出てきたら、その都度ドキュメントを確認
- 使用例（Example）を見る

## 💡 よくある疑問と答え

### Q: `Result<T>`とは？
A: Rustのエラーハンドリング型。成功した場合は`Ok(T)`、失敗した場合は`Err(E)`を返す。

### Q: `?`演算子とは？
A: エラーが発生した場合、そのエラーをそのまま返す。`unwrap()`より安全。

### Q: `async/await`とは？
A: 非同期処理。ファイルI/Oやネットワークリクエストで使用。

### Q: `Mutex`とは？
A: 複数のスレッドから安全にデータにアクセスするためのロック機構。

### Q: テンソルとは？
A: 多次元配列。機械学習でよく使用される。例: 2次元テンソル = 行列

## 🎓 次のステップ

1. **基礎を固める**（1-2週間）
   - Rust Bookを読みながらコードを読む
   - 小さな変更を加えてみる

2. **理解を深める**（2-3週間）
   - 各モジュールを詳細に読む
   - デバッグプリントを追加して動作を確認

3. **実践する**（継続的）
   - バグを修正
   - 機能を追加
   - パフォーマンスを改善

## 📝 チェックリスト

- [ ] `main.rs`を読んで全体構造を理解した
- [ ] Tauriのコマンドシステムを理解した
- [ ] オーディオのデコード/エンコード処理を理解した
- [ ] PyTorchモデルの扱い方を理解した
- [ ] エラーハンドリングの流れを理解した
- [ ] 小さな変更を加えてビルドに成功した

## 🔗 役立つリンク

- [Rust Book（日本語）](https://doc.rust-jp.rs/book-ja/)
- [Tauri ドキュメント](https://tauri.app/v1/guides/)
- [PyTorch ドキュメント](https://pytorch.org/docs/stable/index.html)
- [HTDemucs GitHub](https://github.com/facebookresearch/demucs)

---

**頑張ってください！わからないことがあったら、小さなステップに分解して一つずつ理解していきましょう。** 🚀

