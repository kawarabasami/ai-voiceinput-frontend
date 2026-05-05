use std::sync::{Arc, Mutex};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, SupportedStreamConfig};
use tauri::{AppHandle, Manager};
use webrtc_vad::{Vad, VadMode, SampleRate};
use chrono::Local;
use std::fs;

// Vad is a C struct wrapper, we need to explicitly allow it to be sent across threads if wrapped in Mutex
pub struct SendVad(pub Vad);
unsafe impl Send for SendVad {}
unsafe impl Sync for SendVad {}

pub struct AudioRecorder {
    pub is_recording: bool,
    pub buffer: Vec<i16>,
    pub sample_rate: u32,
    pub silent_frames: u32,
    pub vad_buffer: Vec<i16>,
    pub silence_history: Vec<i16>,
    pub app_handle: Option<AppHandle>,
    pub vad: SendVad,
}

// cpal::Stream を Send + Sync にするためのラッパー
struct SendStream(#[allow(dead_code)] cpal::Stream);
unsafe impl Send for SendStream {}
unsafe impl Sync for SendStream {}

static ACTIVE_STREAM: Mutex<Option<SendStream>> = Mutex::new(None);

impl AudioRecorder {
    pub fn new() -> Self {
        Self {
            is_recording: false,
            buffer: Vec::new(),
            sample_rate: 16000,
            silent_frames: 0,
            vad_buffer: Vec::new(),
            silence_history: Vec::new(),
            app_handle: None,
            vad: SendVad(Vad::new_with_rate_and_mode(SampleRate::Rate16kHz, VadMode::Aggressive)),
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

    let buffer_clone = Arc::clone(&recorder_state);

    let stream = match sample_format {
        SampleFormat::F32 => build_stream::<f32>(&device, &config.into(), buffer_clone),
        SampleFormat::I16 => build_stream_i16(&device, &config.into(), buffer_clone),
        SampleFormat::U16 => build_stream_u16(&device, &config.into(), buffer_clone),
        SampleFormat::I32 => build_stream_i32(&device, &config.into(), buffer_clone),
        SampleFormat::I8 => build_stream_i8(&device, &config.into(), buffer_clone),
        SampleFormat::U8 => build_stream_u8(&device, &config.into(), buffer_clone),
        _ => Err(format!("サポートされていないサンプルフォーマットです: {:?}", sample_format)),
    }?;

    stream.play().map_err(|e| format!("ストリーム開始失敗: {}", e))?;
    
    let mut active_stream = ACTIVE_STREAM.lock().unwrap();
    *active_stream = Some(SendStream(stream));

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

fn process_audio_samples(recorder: &mut AudioRecorder, samples: &[i16]) {
    for &sample in samples {
        recorder.vad_buffer.push(sample);
    }

    while recorder.vad_buffer.len() >= 480 {
        let frame: Vec<i16> = recorder.vad_buffer.drain(..480).collect();
        let is_voice = recorder.vad.0.is_voice_segment(&frame).unwrap_or(true);
        
        if is_voice {
            // 有音フレームになったら、直近の無音履歴（リーディングマージン）を保存バッファに追加
            if !recorder.silence_history.is_empty() {
                recorder.buffer.extend_from_slice(&recorder.silence_history);
                recorder.silence_history.clear();
            }
            
            recorder.buffer.extend_from_slice(&frame);
            recorder.silent_frames = 0;
        } else {
            recorder.silent_frames += 1;
            
            // 無音の開始後300ms(10フレーム)はトレイリングマージンとして保存バッファに残す
            if recorder.silent_frames <= 10 {
                recorder.buffer.extend_from_slice(&frame);
            } else {
                // それ以降の無音は、将来のリーディングマージンとして履歴に保持(最大10フレーム=300ms)
                recorder.silence_history.extend_from_slice(&frame);
                if recorder.silence_history.len() > 480 * 10 {
                    recorder.silence_history.drain(0..480);
                }
            }
        }
    }
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
                        let mut mono_i16_samples = Vec::with_capacity(data.len() / channels);
                        for frame in data.chunks(channels) {
                            let f32_sample = f32::from(frame[0]);
                            let i16_sample = (f32_sample * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
                            mono_i16_samples.push(i16_sample);
                        }
                        process_audio_samples(&mut recorder, &mono_i16_samples);
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
                        let mut mono_i16_samples = Vec::with_capacity(data.len() / channels);
                        for frame in data.chunks(channels) {
                            mono_i16_samples.push(frame[0]);
                        }
                        process_audio_samples(&mut recorder, &mono_i16_samples);
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
                        let mut mono_i16_samples = Vec::with_capacity(data.len() / channels);
                        for frame in data.chunks(channels) {
                            let f32_sample = frame[0] as f32 / i32::MAX as f32;
                            mono_i16_samples.push((f32_sample * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16);
                        }
                        process_audio_samples(&mut recorder, &mono_i16_samples);
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
                        let mut mono_i16_samples = Vec::with_capacity(data.len() / channels);
                        for frame in data.chunks(channels) {
                            let f32_sample = frame[0] as f32 / u16::MAX as f32 * 2.0 - 1.0;
                            mono_i16_samples.push((f32_sample * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16);
                        }
                        process_audio_samples(&mut recorder, &mono_i16_samples);
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
                        let mut mono_i16_samples = Vec::with_capacity(data.len() / channels);
                        for frame in data.chunks(channels) {
                            let f32_sample = frame[0] as f32 / i8::MAX as f32;
                            mono_i16_samples.push((f32_sample * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16);
                        }
                        process_audio_samples(&mut recorder, &mono_i16_samples);
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
                        let mut mono_i16_samples = Vec::with_capacity(data.len() / channels);
                        for frame in data.chunks(channels) {
                            let f32_sample = frame[0] as f32 / u8::MAX as f32 * 2.0 - 1.0;
                            mono_i16_samples.push((f32_sample * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16);
                        }
                        process_audio_samples(&mut recorder, &mono_i16_samples);
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

    // 保存先ディレクトリの決定
    let app_handle = recorder.app_handle.clone().ok_or("AppHandleが見つかりません")?;
    let app_data_dir = app_handle.path().app_data_dir().map_err(|e| e.to_string())?;
    let recordings_dir = app_data_dir.join("recordings");
    fs::create_dir_all(&recordings_dir).map_err(|e| format!("ディレクトリ作成失敗: {}", e))?;

    // ファイル名の生成
    let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let filename = format!("recording_{}.wav", timestamp);
    let file_path = recordings_dir.join(filename);

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer =
        hound::WavWriter::create(&file_path, spec).map_err(|e| format!("WAV作成失敗: {}", e))?;

    for &sample in &buffer {
        writer
            .write_sample(sample)
            .map_err(|e| format!("WAV書き込み失敗: {}", e))?;
    }
    writer.finalize().map_err(|e| format!("WAV確定失敗: {}", e))?;

    // 古い録音データの削除（直近3回分を残す）
    if let Ok(entries) = fs::read_dir(&recordings_dir) {
        let mut files: Vec<_> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("wav"))
            .collect();

        // 作成日時でソート
        files.sort_by_key(|e| e.metadata().and_then(|m| m.created()).ok());

        if files.len() > 3 {
            let num_to_delete = files.len() - 3;
            for i in 0..num_to_delete {
                let _ = fs::remove_file(files[i].path());
            }
        }
    }

    Ok(file_path.to_string_lossy().to_string())
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
