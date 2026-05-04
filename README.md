# Keyboard Player

把键盘变成乐器。按下任意字母键即可发出音符，按住时长决定音符长短（1–3 秒），同时可搭配伴奏文件或内置自动节拍一起演奏。

## 功能

- **按键发音**：字母键映射到三个八度，覆盖 C3 – E6
- **时长感应**：按住越久，音符越长（1–3 秒）
- **丰富音色**：基音 + 二次/三次谐波混合，淡入消除杂音
- **自动节拍**：4/4 拍节奏型，强/弱拍用不同频率模拟鼓点
- **伴奏文件**：支持 mp3 / wav / ogg / flac，循环播放
- **参数可调**：BPM、音符音量、伴奏音量均可通过命令行指定

## 键盘布局

按键盘物理行从低到高排列一条**连续半音阶（C3 – B5，共 36 键）**：

| 区域 | 按键 | 音域 | MIDI |
|------|------|------|------|
| 底行 | `z x c v b n m` | C3 – F#3 | 48–54 |
| 主行 | `a s d f g h j k l` | G3 – D#4 | 55–63 |
| 顶行 | `q w e r t y u i o p` | E4 – C#5 | 64–73 |
| 数字行 | `1 2 3 4 5 6 7 8 9 0` | D5 – B5 | 74–83 |

相邻按键音程恒为半音，跨行音高完全连贯。

## 系统要求

- macOS 10.15+
- Rust 1.70+（通过 [rustup](https://rustup.rs) 安装）
- 「辅助功能」权限（用于全局键盘监听）

## 构建

```bash
cargo build --release
```

二进制文件位于 `target/release/keyboard-player`。

## 使用

```bash
# 纯键盘，无伴奏
./target/release/keyboard-player

# 纯键盘 + 自动节拍（默认 120 BPM）
./target/release/keyboard-player --beat

# 指定伴奏文件（循环播放）
./target/release/keyboard-player --accompaniment song.mp3

# 完整参数
./target/release/keyboard-player \
  --accompaniment song.mp3 \
  --note-volume 0.9 \
  --acc-volume 0.4 \
  --bpm 100
```

### 参数说明

| 参数 | 简写 | 默认值 | 说明 |
|------|------|--------|------|
| `--accompaniment` | `-a` | 无 | 伴奏音频文件路径 |
| `--beat` | — | `false` | 开启自动节拍（无伴奏文件时生效） |
| `--bpm` | — | `120` | 自动节拍 BPM（无伴奏文件时生效） |
| `--note-volume` | — | `0.8` | 音符音量（0.0 – 1.0） |
| `--acc-volume` | — | `0.5` | 伴奏/节拍音量（0.0 – 1.0） |

## macOS 权限

首次运行时若提示「无法监听键盘」，请前往：

**系统设置 → 隐私与安全性 → 辅助功能**

将 `keyboard-player`（或运行它的终端程序，如 Terminal / iTerm2）加入允许列表，然后重新运行。

## 依赖

| Crate | 用途 |
|-------|------|
| [rdev](https://crates.io/crates/rdev) | 全局键盘事件监听 |
| [rodio](https://crates.io/crates/rodio) | 音频播放与合成 |
| [clap](https://crates.io/crates/clap) | 命令行参数解析 |

## 许可证

MIT
