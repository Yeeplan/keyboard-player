use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use clap::Parser;
use rdev::{listen, EventType, Key};
use rodio::source::SineWave;
use rodio::{Decoder, OutputStream, Sink, Source};

/// 把键盘变成乐器，按键发音，可搭配伴奏文件使用
#[derive(Parser, Debug)]
#[command(name = "keyboard-player")]
struct Args {
    /// 伴奏音频文件路径（支持 mp3 / wav / ogg / flac）
    #[arg(short, long)]
    accompaniment: Option<String>,

    /// 自动节拍的 BPM（未指定伴奏文件时生效）
    #[arg(long, default_value = "120")]
    bpm: u32,

    /// 音符音量（0.0 – 1.0）
    #[arg(long, default_value = "0.8")]
    note_volume: f32,

    /// 伴奏 / 节拍音量（0.0 – 1.0）
    #[arg(long, default_value = "0.5")]
    acc_volume: f32,

    /// 开启自动节拍（无伴奏文件时生效，默认关闭）
    #[arg(long)]
    beat: bool,
}

/// 键盘按键 → 音符频率（Hz）
///
/// 按照键盘物理布局从低到高排列一条连续半音阶：
///   底行 z x c v b n m        → C3  – F#3  (MIDI 48–54)
///   主行 a s d f g h j k l   → G3  – D#4  (MIDI 55–63)
///   顶行 q w e r t y u i o p → E4  – C#5  (MIDI 64–73)
///   数字 1 2 3 4 5 6 7 8 9 0 → D5  – B5   (MIDI 74–83)
fn key_to_frequency(key: &Key) -> Option<f32> {
    // f = 440 * 2^((midi - 69) / 12)
    let midi: u8 = match key {
        // ---- 底行: C3(48) → F#3(54) ----
        Key::KeyZ => 48,
        Key::KeyX => 49,
        Key::KeyC => 50,
        Key::KeyV => 51,
        Key::KeyB => 52,
        Key::KeyN => 53,
        Key::KeyM => 54,
        // ---- 主行: G3(55) → D#4(63) ----
        Key::KeyA => 55,
        Key::KeyS => 56,
        Key::KeyD => 57,
        Key::KeyF => 58,
        Key::KeyG => 59,
        Key::KeyH => 60,
        Key::KeyJ => 61,
        Key::KeyK => 62,
        Key::KeyL => 63,
        // ---- 顶行: E4(64) → C#5(73) ----
        Key::KeyQ => 64,
        Key::KeyW => 65,
        Key::KeyE => 66,
        Key::KeyR => 67,
        Key::KeyT => 68,
        Key::KeyY => 69,
        Key::KeyU => 70,
        Key::KeyI => 71,
        Key::KeyO => 72,
        Key::KeyP => 73,
        // ---- 数字行: D5(74) → B5(83) ----
        Key::Num1 => 74,
        Key::Num2 => 75,
        Key::Num3 => 76,
        Key::Num4 => 77,
        Key::Num5 => 78,
        Key::Num6 => 79,
        Key::Num7 => 80,
        Key::Num8 => 81,
        Key::Num9 => 82,
        Key::Num0 => 83,
        _ => return None,
    };
    let freq = 440.0_f32 * 2.0_f32.powf((midi as f32 - 69.0) / 12.0);
    Some(freq)
}

/// 播放带谐波的音符（基音 + 2次谐波 + 3次谐波，使音色更丰富）
fn play_note(handle: &rodio::OutputStreamHandle, freq: f32, duration: Duration, volume: f32) {
    if let Ok(sink) = Sink::try_new(handle) {
        let fund = SineWave::new(freq)
            .take_duration(duration)
            .amplify(volume * 0.70);
        let h2 = SineWave::new(freq * 2.0)
            .take_duration(duration)
            .amplify(volume * 0.20);
        let h3 = SineWave::new(freq * 3.0)
            .take_duration(duration)
            .amplify(volume * 0.08);
        let source = fund
            .mix(h2)
            .mix(h3)
            .fade_in(Duration::from_millis(8));
        sink.append(source);
        sink.detach(); // fire-and-forget，自动播完
    }
}

/// 播放一个短促的节拍音（用于自动节奏）
fn play_tick(handle: &rodio::OutputStreamHandle, freq: f32, volume: f32) {
    if let Ok(sink) = Sink::try_new(handle) {
        let tick = SineWave::new(freq)
            .take_duration(Duration::from_millis(55))
            .amplify(volume)
            .fade_in(Duration::from_millis(4));
        sink.append(tick);
        sink.detach();
    }
}

fn main() {
    let args = Args::parse();

    println!("=== Keyboard Player ===");
    println!("键盘映射（连续半音阶，C3 → B5）:");
    println!("  数字行 (1-0) : D5 – B5  (MIDI 74–83)");
    println!("  顶行   (q-p) : E4 – C#5 (MIDI 64–73)");
    println!("  主行   (a-l) : G3 – D#4 (MIDI 55–63)");
    println!("  底行   (z-m) : C3 – F#3 (MIDI 48–54)");
    println!("按住时间越长，音符越长（1–3 秒）");
    println!("按 Ctrl+C 退出\n");

    if let Some(ref path) = args.accompaniment {
        println!("伴奏文件: {path}");
    } else if args.beat {
        println!("自动节拍: {} BPM（4/4 拍）", args.bpm);
    } else {
        println!("无伴奏 / 无节拍模式（使用 --beat 开启自动节拍，-a 指定伴奏文件）");
    }
    println!();

    // macOS 要求 rdev::listen 在主线程运行，音频处理放在子线程
    let (tx, rx) = mpsc::channel::<(Key, bool, Instant)>();

    let note_volume = args.note_volume.clamp(0.0, 1.0);
    let acc_volume = args.acc_volume.clamp(0.0, 1.0);
    let bpm = args.bpm.clamp(20, 300);
    let accompaniment_path = args.accompaniment.clone();
    let auto_beat_flag = args.beat;

    thread::spawn(move || {
        let (_stream, stream_handle) = match OutputStream::try_default() {
            Ok(v) => v,
            Err(e) => {
                eprintln!("无法打开音频输出设备: {e}");
                return;
            }
        };

        // ---- 启动伴奏 ----
        let use_auto_beat = match accompaniment_path {
            Some(ref path) => match File::open(path) {
                Ok(file) => match Decoder::new(BufReader::new(file)) {
                    Ok(source) => match Sink::try_new(&stream_handle) {
                        Ok(sink) => {
                            sink.set_volume(acc_volume);
                            sink.append(source.repeat_infinite());
                            sink.detach();
                            println!("伴奏已开始播放。");
                            false
                        }
                        Err(e) => {
                            eprintln!("创建伴奏 Sink 失败: {e}，改用自动节拍。");
                            true
                        }
                    },
                    Err(e) => {
                        eprintln!("无法解码 '{path}': {e}，改用自动节拍。");
                        true
                    }
                },
                Err(e) => {
                    eprintln!("无法打开 '{path}': {e}，改用自动节拍。");
                    true
                }
            },
            None => auto_beat_flag,
        };

        let beat_interval = Duration::from_millis(60_000 / bpm as u64);
        let mut beat_count: u32 = 0;
        // 4/4 拍节拍型: 强拍(1) 弱拍(2) 次强拍(3) 弱拍(4)
        // 用不同频率 & 音量模拟鼓点
        let beat_pattern: [(f32, f32); 4] = [
            (880.0, 0.55), // 第1拍 - 强拍（似底鼓）
            (440.0, 0.20), // 第2拍 - 弱拍
            (660.0, 0.38), // 第3拍 - 次强拍（似军鼓）
            (440.0, 0.20), // 第4拍 - 弱拍
        ];

        // key_name -> 按下时刻
        let mut press_times: HashMap<String, Instant> = HashMap::new();

        loop {
            // 有伴奏文件时用长超时，不影响音符响应；
            // 自动节拍时用 beat_interval 超时驱动节奏
            let timeout = if use_auto_beat {
                beat_interval
            } else {
                Duration::from_secs(60)
            };

            match rx.recv_timeout(timeout) {
                Ok((key, pressed, time)) => {
                    let name = format!("{key:?}");
                    if pressed {
                        // or_insert 防止按键重复事件覆盖真正的按下时刻
                        press_times.entry(name).or_insert(time);
                    } else if let Some(press_time) = press_times.remove(&name) {
                        let held = time.duration_since(press_time);
                        // 按住时长映射到 1–3 秒
                        let note_dur = held
                            .max(Duration::from_secs(1))
                            .min(Duration::from_secs(3));
                        if let Some(freq) = key_to_frequency(&key) {
                            play_note(&stream_handle, freq, note_dur, note_volume);
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if use_auto_beat {
                        let idx = (beat_count % 4) as usize;
                        let (freq, vol) = beat_pattern[idx];
                        play_tick(&stream_handle, freq, vol * acc_volume);
                        beat_count += 1;
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    });

    // ---- 主线程：监听键盘（macOS 要求主线程） ----
    if let Err(e) = listen(move |event| match event.event_type {
        EventType::KeyPress(key) => {
            let _ = tx.send((key, true, Instant::now()));
        }
        EventType::KeyRelease(key) => {
            let _ = tx.send((key, false, Instant::now()));
        }
        _ => {}
    }) {
        eprintln!("键盘监听失败: {e:?}");
        eprintln!();
        eprintln!("提示 (macOS)：本程序需要「辅助功能」权限。");
        eprintln!("请前往「系统设置 → 隐私与安全性 → 辅助功能」，将本程序加入允许列表。");
    }
}
