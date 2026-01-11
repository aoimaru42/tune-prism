use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};

pub mod analysis;
pub mod audio;
pub mod error;
pub mod model;

use mime::{Mime, IMAGE, JPEG};
use ndarray::{Array2, ArrayD};

use snafu::{whatever, ResultExt};
use tch::{Device, IndexOp, Kind, Tensor};

use crate::demucs::{
    audio::{decode_file, encode_pcm_to_wav, resample, PcmAudioData},
    error::TorchSnafu,
};

pub use analysis::{detect_bpm, detect_key};
pub use error::{Error, Result};
pub use model::{find_model, models, Demucs, LazyModelLoader};

use self::error::{Id3Snafu, MimeParseSnafu};

pub fn get_available_device() -> Device {
    if tch::utils::has_mps() {
        Device::Mps
    } else if tch::utils::has_cuda() {
        Device::Cuda(0)
    } else {
        Device::Cpu
    }
}

pub fn split_track(model: &Demucs, input_path: &Path, output_dir: &Path) -> Result<Vec<PathBuf>> {
    // let model = &MODEL;
    let track = decode_file(input_path)?;
    let track = resample(track, model.config.sample_rate)?;

    let input_arr: ArrayD<f32> = Array2::from_shape_vec(
        (track.nb_channels, track.length),
        track.samples.into_iter().flatten().collect(),
    )
    .unwrap()
    .into_dyn();

    let mut input_tensor: Tensor = (&input_arr).try_into().context(TorchSnafu)?;

    // HTDemucsの標準的な正規化: 全テンソルに対して平均と標準偏差を計算
    // 以前の実装ではチャンネル次元で平均を取っていたが、全テンソルに対して正規化を行う方が適切
    let mean_val: f32 = input_tensor.mean(Kind::Float).try_into().unwrap_or(0.0);
    let std_val: f32 = input_tensor.std(true).try_into().unwrap_or(1.0);
    
    // ゼロ除算を避けるため、標準偏差が小さい場合は1e-8を使用
    let std_safe_val = if std_val < 1e-8 { 1e-8 } else { std_val };

    input_tensor -= mean_val;
    input_tensor /= std_safe_val;

    let length = input_tensor.size().pop().unwrap();
    let input = input_tensor.reshape([1, 2, length]);

    let mut output = model.apply(input);

    // 非正規化: 標準偏差を掛けて、平均を足す
    output *= std_safe_val;
    output += mean_val;

    // let output = Arc::new(output);

    // OpenMP（libtorchで使用）とrayonの並列処理が競合するため、通常のイテレータを使用
    // WAVファイルのエンコードは比較的軽い処理なので、並列処理がなくても問題ない
    model
        .config
        .sources
        .iter()
        .enumerate()
        .map(|(i, source)| {
            let mut buffer: Vec<Vec<f32>> = vec![vec![0.0; track.length]; model.config.channels];

            let out = output.i((0, i as i64));

            for i in 0..model.config.channels {
                out.i(i as i64).copy_data(&mut buffer[i], track.length);
            }
            (source, buffer)
        })
        .map(|(source, buffer)| {
            // 後処理: ノイズ除去とフィルタリング
            let mut processed_buffer = post_process_stem(&buffer, source, model.config.sample_rate);
            
            // クリック/ポップノイズを除去
            remove_clicks_pops(&mut processed_buffer, model.config.sample_rate);

            let audio_data = PcmAudioData {
                samples: processed_buffer,
                sample_rate: model.config.sample_rate,
                nb_channels: model.config.channels,
                length: track.length,
            };

            let mut stem = source.clone();
            stem.push_str(".wav");
            let path = output_dir.join(stem);

            encode_pcm_to_wav(audio_data, &path)?;

            Ok(path)
        })
        .collect::<Result<Vec<_>>>()
}

/// トラックをVocalとInstrumental（それ以外の組み合わせ）の2つに分離
pub fn split_vocal_instrumental(model: &Demucs, input_path: &Path, output_dir: &Path) -> Result<Vec<PathBuf>> {
    eprintln!("[split_vocal_instrumental] Starting vocal/instrumental separation");
    
    let track = decode_file(input_path)?;
    let track = resample(track, model.config.sample_rate)?;

    let input_arr: ArrayD<f32> = Array2::from_shape_vec(
        (track.nb_channels, track.length),
        track.samples.into_iter().flatten().collect(),
    )
    .unwrap()
    .into_dyn();

    let mut input_tensor: Tensor = (&input_arr).try_into().context(TorchSnafu)?;

    // 正規化
    let mean_val: f32 = input_tensor.mean(Kind::Float).try_into().unwrap_or(0.0);
    let std_val: f32 = input_tensor.std(true).try_into().unwrap_or(1.0);
    let std_safe_val = if std_val < 1e-8 { 1e-8 } else { std_val };

    input_tensor -= mean_val;
    input_tensor /= std_safe_val;

    let length = input_tensor.size().pop().unwrap();
    let input = input_tensor.reshape([1, 2, length]);

    let mut output = model.apply(input);

    // 非正規化
    output *= std_safe_val;
    output += mean_val;

    // Vocalとその他のstemのインデックスを特定
    let vocal_idx = model.config.sources.iter().position(|s| s == "vocals");
    let vocal_idx = vocal_idx.unwrap_or_else(|| {
        eprintln!("[split_vocal_instrumental] WARNING: 'vocals' not found in sources, using first source");
        0
    });

    // Vocal stemを取得
    let mut vocal_buffer: Vec<Vec<f32>> = vec![vec![0.0; track.length]; model.config.channels];
    let vocal_out = output.i((0, vocal_idx as i64));
    for i in 0..model.config.channels {
        vocal_out.i(i as i64).copy_data(&mut vocal_buffer[i], track.length);
    }

    // Instrumental（それ以外すべての組み合わせ）を作成
    let mut instrumental_buffer: Vec<Vec<f32>> = vec![vec![0.0; track.length]; model.config.channels];
    
    for (i, source) in model.config.sources.iter().enumerate() {
        if source != "vocals" {
            let mut stem_buffer: Vec<Vec<f32>> = vec![vec![0.0; track.length]; model.config.channels];
            let stem_out = output.i((0, i as i64));
            for ch in 0..model.config.channels {
                stem_out.i(ch as i64).copy_data(&mut stem_buffer[ch], track.length);
            }
            
            // Instrumentalに加算
            for ch in 0..model.config.channels {
                for j in 0..track.length {
                    instrumental_buffer[ch][j] += stem_buffer[ch][j];
                }
            }
        }
    }

    // Vocalの後処理
    let mut processed_vocal = post_process_stem(&vocal_buffer, "vocals", model.config.sample_rate);
    remove_clicks_pops(&mut processed_vocal, model.config.sample_rate);

    // Instrumentalの後処理（"other"として処理）
    let mut processed_instrumental = post_process_stem(&instrumental_buffer, "other", model.config.sample_rate);
    remove_clicks_pops(&mut processed_instrumental, model.config.sample_rate);

    // WAVファイルとして保存
    let vocal_data = PcmAudioData {
        samples: processed_vocal,
        sample_rate: model.config.sample_rate,
        nb_channels: model.config.channels,
        length: track.length,
    };
    let vocal_path = output_dir.join("vocal.wav");
    encode_pcm_to_wav(vocal_data, &vocal_path)?;
    eprintln!("[split_vocal_instrumental] Saved vocal.wav");

    let instrumental_data = PcmAudioData {
        samples: processed_instrumental,
        sample_rate: model.config.sample_rate,
        nb_channels: model.config.channels,
        length: track.length,
    };
    let instrumental_path = output_dir.join("instrumental.wav");
    encode_pcm_to_wav(instrumental_data, &instrumental_path)?;
    eprintln!("[split_vocal_instrumental] Saved instrumental.wav");

    Ok(vec![vocal_path, instrumental_path])
}

/// 後処理: 各stemタイプに応じたフィルタリング
fn post_process_stem(
    buffer: &[Vec<f32>],
    stem_type: &str,
    sample_rate: usize,
) -> Vec<Vec<f32>> {
    let mut processed = buffer.to_vec();
    
    match stem_type {
        "other" => {
            // other.wavのゴワゴワ感を改善するため、ノイズリダクションを適用
            // ハイパスフィルタで低周波ノイズを除去し、中周波数帯域を強調
            for channel in processed.iter_mut() {
                apply_high_pass_filter(channel, sample_rate, 80.0); // 80Hz以下をカット
                apply_noise_reduction(channel, sample_rate);
            }
        }
        "bass" => {
            // ベースの低周波数帯域を保持（音量調整は行わない - 正確な分離結果を尊重）
            // 必要に応じて、ユーザーが後からミキシング/マスタリングで調整可能
            for channel in processed.iter_mut() {
                // 低周波数を保持しつつ、不要な高周波ノイズを軽減（フィルタリングのみ、音量調整なし）
                apply_low_pass_filter(channel, sample_rate, 400.0); // 400Hz以上をカット（周波数フィルタリングのみ）
            }
        }
        "vocals" => {
            // ボーカルの中周波数帯域を強調（300-3400Hz）
            for channel in processed.iter_mut() {
                apply_band_pass_filter(channel, sample_rate, 300.0, 3400.0);
            }
        }
        "drums" => {
            // ドラムは広帯域を維持
            // 特に処理なし
        }
        "guitar" => {
            // ギターの中高周波数帯域を強調（80-8000Hz）
            for channel in processed.iter_mut() {
                apply_band_pass_filter(channel, sample_rate, 80.0, 8000.0);
            }
        }
        "piano" => {
            // ピアノの広帯域を維持（80-15000Hz）
            for channel in processed.iter_mut() {
                apply_band_pass_filter(channel, sample_rate, 80.0, 15000.0);
            }
        }
        _ => {}
    }
    
    processed
}

/// ハイパスフィルタ: 低周波数をカット
fn apply_high_pass_filter(samples: &mut [f32], sample_rate: usize, cutoff: f32) {
    let rc = 1.0 / (2.0 * std::f32::consts::PI * cutoff);
    let dt = 1.0 / sample_rate as f32;
    let alpha = rc / (rc + dt);
    
    let mut prev_input = 0.0;
    let mut prev_output = 0.0;
    
    for sample in samples.iter_mut() {
        let input = *sample;
        let output = alpha * (prev_output + input - prev_input);
        *sample = output;
        prev_input = input;
        prev_output = output;
    }
}

/// ローパスフィルタ: 高周波数をカット
fn apply_low_pass_filter(samples: &mut [f32], sample_rate: usize, cutoff: f32) {
    let rc = 1.0 / (2.0 * std::f32::consts::PI * cutoff);
    let dt = 1.0 / sample_rate as f32;
    let alpha = dt / (rc + dt);
    
    let mut prev_output = 0.0;
    
    for sample in samples.iter_mut() {
        let output = prev_output + alpha * (*sample - prev_output);
        *sample = output;
        prev_output = output;
    }
}

/// バンドパスフィルタ: 特定の周波数帯域のみを通す
fn apply_band_pass_filter(samples: &mut [f32], sample_rate: usize, low_cut: f32, high_cut: f32) {
    // ハイパスフィルタを適用
    apply_high_pass_filter(samples, sample_rate, low_cut);
    // ローパスフィルタを適用
    apply_low_pass_filter(samples, sample_rate, high_cut);
}

/// 簡易ノイズリダクション: 移動平均を使用してノイズを減らす
fn apply_noise_reduction(samples: &mut [f32], sample_rate: usize) {
    let window_size = (sample_rate as f32 * 0.01) as usize; // 10ms
    if window_size < 2 || samples.len() < window_size * 2 {
        return;
    }
    
    let mut smoothed = samples.to_vec();
    
    for i in window_size..(samples.len() - window_size) {
        let sum: f32 = samples[i - window_size..i + window_size]
            .iter()
            .sum();
        smoothed[i] = sum / (window_size * 2) as f32;
    }
    
    // 元の信号と平滑化された信号をブレンド（ノイズのみを減らす）
    for (original, smooth) in samples.iter_mut().zip(smoothed.iter()) {
        *original = *original * 0.7 + *smooth * 0.3;
    }
}

/// ゲインを適用（音量を増減）
fn apply_gain(samples: &mut [f32], gain: f32) {
    for sample in samples.iter_mut() {
        *sample *= gain;
    }
}

/// ソフトリミッター: クリッピングを防ぎつつ、音を自然に保持
fn apply_soft_limiter(samples: &mut [f32]) {
    let threshold = 0.95; // リミッターの閾値
    let ratio = 0.1; // 圧縮比（閾値を超えた部分をどれだけ圧縮するか）
    
    for sample in samples.iter_mut() {
        let abs_val = sample.abs();
        if abs_val > threshold {
            // ソフトリミッティング: 超過分を圧縮
            let excess = abs_val - threshold;
            let compressed = threshold + excess * ratio;
            *sample = compressed * sample.signum();
        }
    }
}

/// クリック/ポップノイズを除去（デジタルクリップ検出と修正）
fn remove_clicks_pops(samples: &mut [Vec<f32>], sample_rate: usize) {
    let threshold = 0.9; // クリップの閾値
    let window_size = (sample_rate as f32 * 0.001) as usize; // 1ms
    
    for channel in samples.iter_mut() {
        for i in window_size..(channel.len() - window_size) {
            let current = channel[i].abs();
            
            // 急激な変化を検出（クリック/ポップ）
            if current > threshold {
                let prev_avg: f32 = channel[i - window_size..i]
                    .iter()
                    .map(|s| s.abs())
                    .sum::<f32>()
                    / window_size as f32;
                let next_avg: f32 = channel[i + 1..i + window_size + 1]
                    .iter()
                    .map(|s| s.abs())
                    .sum::<f32>()
                    / window_size as f32;
                
                // 前後の平均と大きく異なる場合はクリック/ポップと判断
                if current > prev_avg * 3.0 || current > next_avg * 3.0 {
                    // 前後の平均で補間
                    channel[i] = (prev_avg + next_avg) / 2.0 * channel[i].signum();
                }
            }
        }
    }
}

pub fn get_cover_image(path: &Path, output_dir: &Path) -> Result<Option<PathBuf>> {
    let tags = id3::Tag::read_from_path(path).context(Id3Snafu)?;

    let output = if let Some(image) = tags.pictures().next() {
        let mime: Mime = image.mime_type.parse().context(MimeParseSnafu)?;
        if mime.type_() == IMAGE && mime.subtype() == JPEG {
            let path = output_dir.join("cover.jpg");
            let mut output = whatever!(
                File::options()
                    .create(true)
                    .write(true)
                    .truncate(true)
                    .open(&path),
                "failed to open file"
            );

            whatever!(output.write_all(&image.data), "failed to write to file");
            Ok(Some(path))
        } else {
            Ok(None)
        }
    } else {
        Ok(None)
    };

    dbg!(&output);

    output
}
