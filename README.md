<p align="center"><img src=".github/assets/meetube/icon.png" width="112" alt="PlayTube logo"></p>

<h1 align="center">PlayTube</h1>

<p align="center">
<a href="https://github.com/lth1503ahihi/playtube/releases/latest"><img src="https://img.shields.io/github/v/release/lth1503ahihi/playtube?style=for-the-badge&label=Release&color=e02424" alt="Release"></a>
<a href="LICENSE"><img src="https://img.shields.io/github/license/lth1503ahihi/playtube?style=for-the-badge&label=License&color=0e75b6" alt="License"></a>
</p>

**Trải nghiệm YouTube & YouTube Music mượt mà, hiện đại và không quảng cáo trên Android.**

PlayTube là ứng dụng nghe nhạc và xem video YouTube tối ưu hiệu năng cao với giao diện kính mờ (Liquid Frosted Glass), tích hợp lõi xử lý âm thanh Rust Native DSP Core (ARM64 SIMD), AI Karaoke tách lời trực tiếp trên máy, hỗ trợ tần số quét 120Hz và tìm kiếm giọng nói Tiếng Việt chuẩn xác.

<p align="center"><img src=".github/assets/meetube/showcase.webp" width="100%" alt="PlayTube Showcase"></p>

<p align="center"><a href="https://github.com/lth1503ahihi/playtube/releases/latest"><b>⬇ Tải về phiên bản PlayTube mới nhất (ARM64)</b></a></p>

---

## 🌟 Điểm nổi bật của PlayTube

* 🚀 **Lõi Rust Native DSP Core (ARM64 SIMD):** Tích hợp bộ giải thuật BS2B Binaural Spatial Audio tạo không gian âm thanh vòm sống động và Bass Exciter tăng cường dải trầm mạnh mẽ.
* ⚡ **Phát video tức thì (Zero Latency):** Bộ đệm Cronet 64KB, Start Buffer 200ms, Preload 4s giúp chuyển bài gần như 0ms; tối ưu luồng Shorts vào là phát ngay, không độ trễ.
* 🔄 **Mượt mà 120Hz / 90Hz:** Kích hoạt mặc định tần số quét cao nhất của màn hình, cuộn lướt như nhung.
* 🎤 **PlayTune AI Karaoke & PlayTuneMix:** Tách giọng hát ca sĩ trực tiếp trên điện thoại bằng mô hình AI (chạy offline 100%, bảo mật tuyệt đối) cùng cơ chế chuyển bài thông minh hòa âm DJ mượt mà.
* 🔍 **Tìm kiếm giọng nói Song ngữ:** Hỗ trợ chuẩn xác Tiếng Việt (`vi-VN`) và Tiếng Anh (`en-US`), nhận diện nhanh và chính xác.
* 🔍 **Thu phóng 8x linh hoạt:** Dùng hai ngón tay zoom video mượt mà lên tới 8x như ứng dụng YouTube gốc.
* 💎 **Giao diện Liquid Glass:** Thiết kế kính mờ sang trọng, thanh tab nổi, hiệu ứng mở app tinh tế.
* 🇻🇳 **100% Tiếng Việt tự nhiên:** Toàn bộ cài đặt, tính năng và câu nói truyền cảm hứng hàng ngày được bản địa hóa chuẩn xác.
* 🔄 **Tự động cập nhật trực tiếp:** Tích hợp kiểm tra và cập nhật ứng dụng in-app tự động từ GitHub Releases chính chủ.

---

## 📱 Tính năng chi tiết

### 🎵 PlayTune - Trình phát nhạc chuyên nghiệp
Chạm vào logo PlayTube trên thanh trang chủ để chuyển sang chế độ YouTube Music:
- **AI Karaoke:** Tách lời bài hát thời gian thực trên chip điện thoại, thỏa sức hát karaoke.
- **PlayTuneMix:** Hòa âm chuyển bài liền mạch không ngắt quãng (Crossfade thông minh tự động căn nhịp beat).
- **Lời bài hát đồng bộ (Synced Lyrics):** Chạy chữ từng câu chuẩn xác theo thời gian bài hát.
- **Tùy chỉnh âm thanh đỉnh cao:** Bộ cân bằng Equalizer đa băng tần, âm thanh vòm không gian MIT KEMAR.

### 🎬 Xem video & Shorts tối ưu
- **Không quảng cáo:** Chặn toàn bộ quảng cáo video và biểu ngữ phiền toái.
- **Shorts mượt mà:** Chuyển đổi và vuốt xem Shorts không độ trễ, tự động đệm khung hình video tiếp theo.
- **Phát trong nền (Background Playback) & PiP:** Tiếp tục nghe khi tắt màn hình hoặc chuyển sang ứng dụng khác.
- **Tải xuống ngoại tuyến:** Hỗ trợ tải video chất lượng cao lên tới 4K và tải file âm thanh kèm lời.

---

## 🛠 Hướng dẫn Build từ nguồn

Yêu cầu môi trường:
- Android SDK & NDK r25+
- Rust & `cargo-ndk` (`rustup target add aarch64-linux-android`)
- JDK 21

```bash
# 1. Clone repository kèm submodules
git clone --recursive https://github.com/lth1503ahihi/playtube.git
cd playtube

# 2. Build Rust DSP Core cho ARM64
cargo install cargo-ndk
cd native/playtube-core
cargo ndk --platform 24 -t arm64-v8a -o ../../mobile/src/stmobile/jniLibs build --release
cd ../..

# 3. Build APK Debug
./gradlew :mobile:assembleStmobileDebug

# APK đầu ra: mobile/build/outputs/apk/stmobile/debug/
```

---

## 🤝 Lời cảm ơn (Credits)

PlayTube được phát triển dựa trên nền tảng của:
- [MeeTube](https://github.com/yashoncode/meetube) bởi [@yashoncode](https://github.com/yashoncode)
- [SmartTube](https://github.com/yuliskov/SmartTube) bởi [@yuliskov](https://github.com/yuliskov)
- [NewTube](https://github.com/aleixrodriala/newtube) bởi [@aleixrodriala](https://github.com/aleixrodriala)
- Hồ sơ cân bằng âm thanh tai nghe từ [AutoEq](https://github.com/jaakkopasanen/AutoEq) bởi [@jaakkopasanen](https://github.com/jaakkopasanen)

---

## 📄 Bản quyền (License)

Dự án được phân phối dưới giấy phép [MIT License](LICENSE).
