use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, SupportedStreamConfig};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

pub struct AudioRecorder {
    is_recording: bool,
    buffer: Vec<f32>,
    sample_rate: u32,
}

// cpal::Stream を Send + Sync にするためのラッパー
struct SendStream(#[allow(dead_code)] cpal::Stream);
unsafe impl Send for SendStream {}
unsafe impl Sync for SendStream {}

static ACTIVE_STREAM: Mutex<Option<SendStream>> = Mutex::new(None);
static RECORDING_READY: OnceLock<(Mutex<bool>, Condvar)> = OnceLock::new();

fn recording_ready_signal() -> &'static (Mutex<bool>, Condvar) {
    RECORDING_READY.get_or_init(|| (Mutex::new(false), Condvar::new()))
}

fn reset_recording_ready() {
    let (ready, _) = recording_ready_signal();
    if let Ok(mut ready) = ready.lock() {
        *ready = false;
    }
}

fn notify_recording_ready() {
    let (ready, condvar) = recording_ready_signal();
    if let Ok(mut ready) = ready.lock() {
        if !*ready {
            *ready = true;
            condvar.notify_all();
        }
    }
}

fn wait_recording_ready() -> Result<(), String> {
    let (ready, condvar) = recording_ready_signal();
    let mut ready = ready.lock().map_err(|e| e.to_string())?;
    let timeout = Duration::from_secs(2);
    let start = Instant::now();

    while !*ready {
        let elapsed = start.elapsed();
        if elapsed >= timeout {
            break;
        }
        let result = condvar
            .wait_timeout(ready, timeout - elapsed)
            .map_err(|e| e.to_string())?;
        ready = result.0;
    }

    if *ready {
        Ok(())
    } else {
        Err("録音デバイスの開始確認がタイムアウトしました".to_string())
    }
}

impl AudioRecorder {
    pub fn new() -> Self {
        Self {
            is_recording: false,
            buffer: Vec::new(),
            sample_rate: 16000,
        }
    }

    #[allow(dead_code)]
    pub fn is_recording(&self) -> bool {
        self.is_recording
    }
}

pub fn start_recording(
    recorder_state: Arc<Mutex<AudioRecorder>>,
    device_index: usize,
) -> Result<(), String> {
    let mut recorder = recorder_state.lock().map_err(|e| e.to_string())?;
    if recorder.is_recording {
        return Ok(());
    }

    let host = cpal::default_host();
    let devices: Vec<Device> = host
        .input_devices()
        .map_err(|e| format!("デバイス列挙失敗: {}", e))?
        .collect();

    let device = devices
        .into_iter()
        .nth(device_index)
        .or_else(|| host.default_input_device())
        .ok_or_else(|| "マイクデバイスが見つかりません".to_string())?;

    let config = find_config_16khz(&device)?;
    let sample_format = config.sample_format();

    recorder.buffer.clear();
    recorder.sample_rate = config.sample_rate().0;
    recorder.is_recording = true;
    reset_recording_ready();

    let buffer_clone = Arc::clone(&recorder_state);

    let stream = match sample_format {
        SampleFormat::F32 => build_stream::<f32>(&device, &config.into(), buffer_clone),
        SampleFormat::I16 => build_stream_i16(&device, &config.into(), buffer_clone),
        SampleFormat::U16 => build_stream_u16(&device, &config.into(), buffer_clone),
        SampleFormat::I32 => build_stream_i32(&device, &config.into(), buffer_clone),
        SampleFormat::I8 => build_stream_i8(&device, &config.into(), buffer_clone),
        SampleFormat::U8 => build_stream_u8(&device, &config.into(), buffer_clone),
        _ => Err(format!(
            "サポートされていないサンプルフォーマットです: {:?}",
            sample_format
        )),
    }?;

    drop(recorder);

    if let Err(e) = stream.play() {
        if let Ok(mut recorder) = recorder_state.lock() {
            recorder.is_recording = false;
            recorder.buffer.clear();
        }
        return Err(format!("ストリーム開始失敗: {}", e));
    }

    let mut active_stream = ACTIVE_STREAM.lock().unwrap();
    *active_stream = Some(SendStream(stream));
    drop(active_stream);

    if let Err(e) = wait_recording_ready() {
        if let Ok(mut active_stream) = ACTIVE_STREAM.lock() {
            *active_stream = None;
        }
        if let Ok(mut recorder) = recorder_state.lock() {
            recorder.is_recording = false;
            recorder.buffer.clear();
        }
        return Err(e);
    }

    Ok(())
}

fn find_config_16khz(device: &Device) -> Result<SupportedStreamConfig, String> {
    let supported = device
        .supported_input_configs()
        .map_err(|e| format!("設定取得失敗: {}", e))?;

    let target_rate = cpal::SampleRate(16000);

    for range in supported {
        if range.min_sample_rate() <= target_rate && target_rate <= range.max_sample_rate() {
            return Ok(range.with_sample_rate(target_rate));
        }
    }

    device
        .default_input_config()
        .map_err(|e| format!("デフォルト設定取得失敗: {}", e))
}

fn build_stream<T>(
    device: &Device,
    config: &cpal::StreamConfig,
    state: Arc<Mutex<AudioRecorder>>,
) -> Result<cpal::Stream, String>
where
    T: cpal::Sample + cpal::SizedSample + hound::Sample,
    f32: From<T>,
{
    let channels = config.channels as usize;
    let stream = device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                if let Ok(mut recorder) = state.lock() {
                    if recorder.is_recording {
                        let had_samples = !recorder.buffer.is_empty();
                        for frame in data.chunks(channels) {
                            recorder.buffer.push(f32::from(frame[0]));
                        }
                        if !had_samples && !recorder.buffer.is_empty() {
                            notify_recording_ready();
                        }
                    }
                }
            },
            |err| eprintln!("[AudioRecorder] ストリームエラー: {}", err),
            None,
        )
        .map_err(|e| format!("ストリーム作成失敗: {}", e))?;
    Ok(stream)
}

fn build_stream_i16(
    device: &Device,
    config: &cpal::StreamConfig,
    state: Arc<Mutex<AudioRecorder>>,
) -> Result<cpal::Stream, String> {
    let channels = config.channels as usize;
    let stream = device
        .build_input_stream(
            config,
            move |data: &[i16], _| {
                if let Ok(mut recorder) = state.lock() {
                    if recorder.is_recording {
                        let had_samples = !recorder.buffer.is_empty();
                        for frame in data.chunks(channels) {
                            recorder.buffer.push(frame[0] as f32 / i16::MAX as f32);
                        }
                        if !had_samples && !recorder.buffer.is_empty() {
                            notify_recording_ready();
                        }
                    }
                }
            },
            |err| eprintln!("[AudioRecorder] ストリームエラー: {}", err),
            None,
        )
        .map_err(|e| format!("ストリーム作成失敗: {}", e))?;
    Ok(stream)
}

fn build_stream_i32(
    device: &Device,
    config: &cpal::StreamConfig,
    state: Arc<Mutex<AudioRecorder>>,
) -> Result<cpal::Stream, String> {
    let channels = config.channels as usize;
    let stream = device
        .build_input_stream(
            config,
            move |data: &[i32], _| {
                if let Ok(mut recorder) = state.lock() {
                    if recorder.is_recording {
                        let had_samples = !recorder.buffer.is_empty();
                        for frame in data.chunks(channels) {
                            recorder.buffer.push(frame[0] as f32 / i32::MAX as f32);
                        }
                        if !had_samples && !recorder.buffer.is_empty() {
                            notify_recording_ready();
                        }
                    }
                }
            },
            |err| eprintln!("[AudioRecorder] ストリームエラー: {}", err),
            None,
        )
        .map_err(|e| format!("ストリーム作成失敗: {}", e))?;
    Ok(stream)
}

fn build_stream_u16(
    device: &Device,
    config: &cpal::StreamConfig,
    state: Arc<Mutex<AudioRecorder>>,
) -> Result<cpal::Stream, String> {
    let channels = config.channels as usize;
    let stream = device
        .build_input_stream(
            config,
            move |data: &[u16], _| {
                if let Ok(mut recorder) = state.lock() {
                    if recorder.is_recording {
                        let had_samples = !recorder.buffer.is_empty();
                        for frame in data.chunks(channels) {
                            let s = frame[0] as f32 / u16::MAX as f32 * 2.0 - 1.0;
                            recorder.buffer.push(s);
                        }
                        if !had_samples && !recorder.buffer.is_empty() {
                            notify_recording_ready();
                        }
                    }
                }
            },
            |err| eprintln!("[AudioRecorder] ストリームエラー: {}", err),
            None,
        )
        .map_err(|e| format!("ストリーム作成失敗: {}", e))?;
    Ok(stream)
}

fn build_stream_i8(
    device: &Device,
    config: &cpal::StreamConfig,
    state: Arc<Mutex<AudioRecorder>>,
) -> Result<cpal::Stream, String> {
    let channels = config.channels as usize;
    let stream = device
        .build_input_stream(
            config,
            move |data: &[i8], _| {
                if let Ok(mut recorder) = state.lock() {
                    if recorder.is_recording {
                        let had_samples = !recorder.buffer.is_empty();
                        for frame in data.chunks(channels) {
                            recorder.buffer.push(frame[0] as f32 / i8::MAX as f32);
                        }
                        if !had_samples && !recorder.buffer.is_empty() {
                            notify_recording_ready();
                        }
                    }
                }
            },
            |err| eprintln!("[AudioRecorder] ストリームエラー: {}", err),
            None,
        )
        .map_err(|e| format!("ストリーム作成失敗: {}", e))?;
    Ok(stream)
}

fn build_stream_u8(
    device: &Device,
    config: &cpal::StreamConfig,
    state: Arc<Mutex<AudioRecorder>>,
) -> Result<cpal::Stream, String> {
    let channels = config.channels as usize;
    let stream = device
        .build_input_stream(
            config,
            move |data: &[u8], _| {
                if let Ok(mut recorder) = state.lock() {
                    if recorder.is_recording {
                        let had_samples = !recorder.buffer.is_empty();
                        for frame in data.chunks(channels) {
                            let s = frame[0] as f32 / u8::MAX as f32 * 2.0 - 1.0;
                            recorder.buffer.push(s);
                        }
                        if !had_samples && !recorder.buffer.is_empty() {
                            notify_recording_ready();
                        }
                    }
                }
            },
            |err| eprintln!("[AudioRecorder] ストリームエラー: {}", err),
            None,
        )
        .map_err(|e| format!("ストリーム作成失敗: {}", e))?;
    Ok(stream)
}

pub fn stop_recording(recorder_state: Arc<Mutex<AudioRecorder>>) -> Result<String, String> {
    {
        let mut active_stream = ACTIVE_STREAM.lock().unwrap();
        *active_stream = None;
    }

    let mut recorder = recorder_state.lock().map_err(|e| e.to_string())?;
    if !recorder.is_recording {
        return Err("録音中ではありません".to_string());
    }

    recorder.is_recording = false;

    let buffer = recorder.buffer.clone();
    let sample_rate = recorder.sample_rate;
    recorder.buffer.clear();

    if buffer.is_empty() {
        return Err("録音データが空です".to_string());
    }

    let temp_path = recording_path()?;
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer =
        hound::WavWriter::create(&temp_path, spec).map_err(|e| format!("WAV作成失敗: {}", e))?;

    for sample in &buffer {
        let s = (sample * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
        writer
            .write_sample(s)
            .map_err(|e| format!("WAV書き込み失敗: {}", e))?;
    }
    writer
        .finalize()
        .map_err(|e| format!("WAV確定失敗: {}", e))?;
    if let Err(e) = cleanup_old_recordings(&temp_path) {
        eprintln!(
            "[AudioRecorder] 古い録音ファイルのクリーンアップに失敗しました: {}",
            e
        );
    }

    Ok(temp_path.to_string_lossy().to_string())
}

fn recordings_dir() -> PathBuf {
    std::env::temp_dir().join("voice_input_app")
}

fn recording_path() -> Result<PathBuf, String> {
    let dir = recordings_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("録音ディレクトリの作成失敗: {}", e))?;
    Ok(dir.join(format!("voice_input_{}.wav", uuid::Uuid::new_v4())))
}

fn cleanup_old_recordings(current_path: &Path) -> Result<(), String> {
    let recordings_dir = current_path
        .parent()
        .ok_or_else(|| "録音ディレクトリを特定できません".to_string())?;
    let mut files = std::fs::read_dir(recordings_dir)
        .map_err(|e| format!("録音ディレクトリの読み取り失敗: {}", e))?
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .path()
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.starts_with("voice_input_") && name.ends_with(".wav"))
                .unwrap_or(false)
        })
        .filter_map(|entry| {
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((entry.path(), modified))
        })
        .collect::<Vec<_>>();

    files.sort_by_key(|(_, modified)| *modified);

    let files_to_delete = files.len().saturating_sub(3);
    for (path, _) in files.into_iter().take(files_to_delete) {
        if path != current_path {
            let _ = std::fs::remove_file(path);
        }
    }

    Ok(())
}

pub fn trim_recording_silence(path: String) -> Result<String, String> {
    const FRAME_MS: usize = 20;
    const MIN_SILENCE_MS: usize = 350;
    const PADDING_MS: usize = 120;
    const MIN_THRESHOLD: f32 = 0.010;
    const MAX_THRESHOLD: f32 = 0.040;

    let requested_path = PathBuf::from(path);
    let wav_path = requested_path
        .canonicalize()
        .map_err(|e| format!("Audio file path resolve failed: {}", e))?;
    let temp_dir = recordings_dir()
        .canonicalize()
        .map_err(|e| format!("Recordings directory resolve failed: {}", e))?;
    let file_name = wav_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "Invalid audio file name".to_string())?;

    if !wav_path.starts_with(&temp_dir)
        || !file_name.starts_with("voice_input_")
        || !file_name.ends_with(".wav")
    {
        return Err("Audio file is outside the allowed recordings directory".to_string());
    }

    let mut reader =
        hound::WavReader::open(&wav_path).map_err(|e| format!("WAV read failed: {}", e))?;
    let spec = reader.spec();

    if spec.channels != 1
        || spec.sample_format != hound::SampleFormat::Int
        || spec.bits_per_sample != 16
    {
        return Err("Unsupported WAV format for silence trimming".to_string());
    }

    let samples = reader
        .samples::<i16>()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("WAV sample read failed: {}", e))?;
    drop(reader);

    if samples.is_empty() {
        return Ok(wav_path.to_string_lossy().to_string());
    }

    let sample_rate = spec.sample_rate as usize;
    let frame_size = (sample_rate * FRAME_MS / 1000).max(1);
    let min_silence_samples = sample_rate * MIN_SILENCE_MS / 1000;
    let padding_samples = sample_rate * PADDING_MS / 1000;

    let i16_max_f32 = i16::MAX as f32;
    let normalization_factor = i16_max_f32 * i16_max_f32;
    let frame_rms = samples
        .chunks(frame_size)
        .map(|frame| {
            let sum_squares = frame
                .iter()
                .map(|sample| {
                    let s = *sample as f32;
                    s * s
                })
                .sum::<f32>();
            (sum_squares / (frame.len() as f32 * normalization_factor)).sqrt()
        })
        .collect::<Vec<_>>();

    let mut sorted_rms = frame_rms.clone();
    sorted_rms.sort_by(|a, b| a.total_cmp(b));
    let noise_floor = sorted_rms[sorted_rms.len() / 5];
    let threshold = (noise_floor * 3.0).clamp(MIN_THRESHOLD, MAX_THRESHOLD);

    let mut ranges = Vec::<(usize, usize)>::new();
    let mut current_start: Option<usize> = None;

    for (frame_index, rms) in frame_rms.iter().enumerate() {
        if *rms >= threshold {
            if current_start.is_none() {
                current_start = Some(frame_index * frame_size);
            }
        } else if let Some(start) = current_start.take() {
            ranges.push((start, (frame_index * frame_size).min(samples.len())));
        }
    }

    if let Some(start) = current_start {
        ranges.push((start, samples.len()));
    }

    if ranges.is_empty() {
        return Ok(wav_path.to_string_lossy().to_string());
    }

    let padded_ranges = ranges
        .into_iter()
        .map(|(start, end)| {
            (
                start.saturating_sub(padding_samples),
                (end + padding_samples).min(samples.len()),
            )
        })
        .collect::<Vec<_>>();

    let mut merged_ranges = Vec::<(usize, usize)>::new();
    for (start, end) in padded_ranges {
        if let Some((_, previous_end)) = merged_ranges.last_mut() {
            if start.saturating_sub(*previous_end) < min_silence_samples {
                *previous_end = (*previous_end).max(end);
                continue;
            }
        }
        merged_ranges.push((start, end));
    }

    let trimmed_samples = merged_ranges
        .iter()
        .flat_map(|(start, end)| samples[*start..*end].iter().copied())
        .collect::<Vec<_>>();

    if trimmed_samples.len() >= samples.len() {
        return Ok(wav_path.to_string_lossy().to_string());
    }

    let temp_path = wav_path.with_extension("trimmed.tmp.wav");
    {
        let mut writer = hound::WavWriter::create(&temp_path, spec)
            .map_err(|e| format!("Trimmed WAV create failed: {}", e))?;
        for sample in trimmed_samples {
            writer
                .write_sample(sample)
                .map_err(|e| format!("Trimmed WAV write failed: {}", e))?;
        }
        writer
            .finalize()
            .map_err(|e| format!("Trimmed WAV finalize failed: {}", e))?;
    }

    replace_recording_file(&temp_path, &wav_path)?;

    Ok(wav_path.to_string_lossy().to_string())
}

fn replace_recording_file(temp_path: &Path, wav_path: &Path) -> Result<(), String> {
    match std::fs::rename(temp_path, wav_path) {
        Ok(()) => Ok(()),
        Err(rename_error) => {
            std::fs::copy(temp_path, wav_path).map_err(|copy_error| {
                let _ = std::fs::remove_file(temp_path);
                format!(
                    "Trimmed WAV replace failed: rename: {}; copy: {}",
                    rename_error, copy_error
                )
            })?;
            std::fs::remove_file(temp_path)
                .map_err(|e| format!("Trimmed WAV cleanup failed: {}", e))?;
            Ok(())
        }
    }
}

pub fn get_recording_audio(path: String) -> Result<Vec<u8>, String> {
    let requested_path = PathBuf::from(path);
    let canonical_path = requested_path
        .canonicalize()
        .map_err(|e| format!("録音ファイルの解決失敗: {}", e))?;
    let temp_dir = recordings_dir()
        .canonicalize()
        .map_err(|e| format!("録音ディレクトリの解決失敗: {}", e))?;
    let file_name = canonical_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "録音ファイル名が不正です".to_string())?;

    if !canonical_path.starts_with(&temp_dir)
        || !file_name.starts_with("voice_input_")
        || !file_name.ends_with(".wav")
    {
        return Err("許可されていない録音ファイルです".to_string());
    }

    std::fs::read(canonical_path).map_err(|e| format!("録音ファイルの読み取り失敗: {}", e))
}

pub fn get_input_devices() -> Result<Vec<(usize, String)>, String> {
    let host = cpal::default_host();
    let devices = host
        .input_devices()
        .map_err(|e| format!("デバイス列挙失敗: {}", e))?;

    Ok(devices
        .enumerate()
        .map(|(i, d)| {
            let name = d.name().unwrap_or_else(|_| format!("デバイス {}", i));
            (i, name)
        })
        .collect())
}
