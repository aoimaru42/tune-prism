# Tune Prism リリース手順

## 1. バージョン番号の更新（オプション）

リリース前にバージョン番号を更新することを推奨します。

以下のファイルでバージョンを統一してください：
- `package.json` の `version` フィールド
- `src-tauri/tauri.conf.json` の `package.version` フィールド
- `src-tauri/Cargo.toml` の `version` フィールド

例：`1.0.0` に更新する場合：

```bash
# package.json
"version": "1.0.0"

# src-tauri/tauri.conf.json
"package": {
  "version": "1.0.0"
}

# src-tauri/Cargo.toml
[package]
version = "1.0.0"
```

## 2. リリースビルドの実行

以下のコマンドでリリース用のビルドを実行します：

```bash
npm run tauri:build
```

このコマンドは以下を実行します：
1. フロントエンド（React）のビルド（`npm run build`）
2. バックエンド（Rust）のリリースビルド
3. アプリケーションバンドルの作成

## 3. ビルド成果物の確認

ビルドが完了すると、以下の場所に成果物が生成されます：

### macOS
- **アプリケーションバンドル**: `src-tauri/target/release/bundle/macos/Tune Prism.app`
- **DMGファイル**: `src-tauri/target/release/bundle/dmg/Tune Prism_1.0.0_x64.dmg`（バージョン番号とアーキテクチャにより異なります）

### ファイルサイズについて
- `.app`バンドルには、アプリケーション本体、libtorchライブラリ、モデルファイル（`htdemucs.pt`など）が含まれます
- モデルファイル（約350MB）を含むため、バンドルサイズは大きくなります（約500MB〜1GB程度）

## 4. 配布方法

### macOS向け

#### 方法1: DMGファイルの配布（推奨）
1. `src-tauri/target/release/bundle/dmg/` に生成された `.dmg` ファイルを使用
2. GitHub Releasesやウェブサイトで配布

#### 方法2: .appバンドルの直接配布
1. `src-tauri/target/release/bundle/macos/Tune Prism.app` をZIP圧縮
2. ユーザーは展開後にアプリケーションフォルダにコピーして使用

**注意**: macOSでは、未署名のアプリケーションを初めて開く際にセキュリティ警告が表示されます。解決方法：
- 右クリック → 「開く」を選択
- または、`xattr -cr "Tune Prism.app"` コマンドを実行

#### 方法3: コード署名と公証（本格的な配布向け）
App Storeや公的な配布を行う場合、Apple Developerアカウントでコード署名と公証が必要です：
1. Apple Developerアカウントの取得
2. 証明書の作成
3. `tauri.conf.json`に署名設定を追加
4. ビルド後、公証プロセスを実行

### サイズ最適化（オプション）

アプリサイズを削減したい場合：
- 不要なモデルファイルを除外（`htdemucs_6s.pt`が不要な場合など）
- リソースファイルの最適化
- ただし、モデルファイルは必要なので、大幅な削減は困難です

## 5. トラブルシューティング

### ビルドエラーが発生した場合

1. **libtorchが見つからない**
   ```bash
   export LIBTORCH=$(pwd)/libtorch
   export DYLD_LIBRARY_PATH=$(pwd)/libtorch/lib:${DYLD_LIBRARY_PATH:-}
   ```

2. **モデルファイルが見つからない**
   - `src-tauri/models/` に `htdemucs.pt` が存在することを確認
   - `get_models.sh` を実行してモデルをダウンロード

3. **メモリ不足**
   - リリースビルドは時間がかかります（10〜30分程度）
   - 十分なメモリを確保してください（8GB以上推奨）

## 6. 開発ビルドとの違い

- **開発ビルド** (`npm run tauri:dev`): ホットリロード、デバッグ情報あり、最適化なし
- **リリースビルド** (`npm run tauri:build`): 最適化済み、デバッグ情報なし、バンドル作成

リリースビルドは実行速度が速く、ファイルサイズも小さくなります。
