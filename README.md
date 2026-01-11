# Tune Prism

> このリポジトリは [hedonhermdev/tune-prism](https://github.com/hedonhermdev/tune-prism) のフォークです。

楽曲を4つのステム（ボーカル、ドラム、ベース、その他）に分割するアプリケーション。FacebookのHTDemucsモデルをベースにしています。

## 技術スタック

Rust、Tauri、PyTorch、Reactで構築されています。

## デモ

トラックをドラッグ＆ドロップして、ステムを抽出し、ステムをドラッグして出力します。

https://github.com/user-attachments/assets/584cf59e-ef4b-4f24-913d-dc52d7549609

## 試す

M1 MacでMacOSを実行している場合、リリースページに事前ビルドされたバイナリがあります。現在、これがビルドしてテストした唯一のプラットフォームです。他のプラットフォームへの移植には作業が必要で、MacBookしか持っていないためです。LinuxやWindowsマシンでアプリを実行できるようにしてくれるなら、喜んでPRを受け入れます。

## ローカルでビルド

これらの手順は、MacOSを実行しているM1 Macbook Proで動作することが確認されています。

### 必要なもの

#### Rust and Cargo
[rustup](rustup.rs)を使用してRustをインストールできます。MSRVはわかりませんが、アプリをビルドする際に`v1.79.0`を使用しました。

```bash
$ rustc --version
rustc 1.79.0 (129f3b996 2024-06-10)

$ cargo --version
cargo 1.79.0 (ffa9cf99a 2024-06-03)
```

#### Node and NPM
```bash
$ brew install node@20

$ node --version 
v20.14.0

$ npm --version
10.7.0
```

#### PyTorch

`libtorch`を使用するか、PYTORCHインストールへのパスを提供できます。直接`libtorch`を使用する方が簡単でした。

```bash
$ wget https://download.pytorch.org/libtorch/cpu/libtorch-macos-arm64-2.2.0.zip
$ unzip libtorch-macos-arm64-2.2.0.zip
```

#### その他の依存関係

```bash
$ brew install libomp
```

### アプリのビルド

- リポジトリをクローン
```bash
$ git clone https://github.com/aoimaru42/tune-prism && cd tune-prism
```

- npmの依存関係をインストール
```bash
$ npm install
```

- モデルをダウンロード
`get_models.sh`スクリプトを使用してモデルをダウンロードできます

```bash
$ ./get_models.sh
```

- `libtorch`をリポジトリにコピー
```
$ cp PATH_TO_LIBTORCH ./libtorch
$ export LIBTORCH=$(realpath ./libtorch) 
```

これで、アプリのビルドを開始する準備が整いました。

```bash
$ npm run tauri build
$ npm run tauri dev # 開発用
```

# コントリビューション

PRを開いてください :)
