# System Monitor

Mac 메뉴 막대 / Windows 트레이에 상주하면서 CPU, GPU, 메모리, 디스크 상태를 1초마다 보여주는 작은 위젯입니다.

```
┌─ System Monitor ──────── 📌 ✕ ┐
│ CPU   ▂▃▅▃▂▄▆▅▃▂          37% │
│       ▮▮▯▮▯▮▮▯▮▯  (코어별)     │
│ GPU   ▁▁▂▁▃▂▁▁▂▁          12% │
│ MEM              9.2 / 16.0 GB │
│ DISK           312.0 / 500.0 GB│
└────────────────────────────────┘
```

## 기능

- **CPU**: 전체 사용률, 최근 60초 그래프, 코어별 막대
- **GPU**: 사용률과 최근 60초 그래프. 읽을 수 없는 기기에서는 `N/A`
- **MEM / DISK**: 사용량과 막대. 90% 이상이면 경고색
- 메뉴 막대(Mac)에 `CPU 37%  MEM 58%` 요약 표시, Windows에서는 트레이 아이콘 툴팁
- 트레이 아이콘 왼쪽 클릭으로 위젯 열기/닫기, 오른쪽 클릭 메뉴 `열기` / `종료`
- 📌 버튼으로 항상 위에 고정, ✕ 버튼으로 숨기기 (앱은 트레이에 남음)
- 관리자 권한이 필요 없습니다

첫 버전에 없는 것: 프로세스별 목록, 온도·팬 속도, 경고 알림, 기록 저장.

## 개발 실행

필요한 것: [Rust](https://rustup.rs), Node.js 20 이상.

```sh
npm install
npm run tauri dev
```

## 테스트

```sh
cargo test --manifest-path src-tauri/Cargo.toml
npm test
```

## 배포 빌드

```sh
npm run tauri build
```

- macOS: `src-tauri/target/release/bundle/macos/System Monitor.app` 과 `.dmg`
- Windows: `src-tauri\target\release\bundle\` 아래 `.msi` / `.exe` 설치 파일

### Windows에서 빌드하기

Windows 빌드는 Windows PC에서 해야 합니다.

1. [Rust](https://rustup.rs) 설치 (설치 중 안내되는 **MSVC C++ Build Tools** 포함)
2. Node.js 설치
3. WebView2 런타임 (Windows 10 최신 버전과 Windows 11에는 기본 포함)
4. 저장소 폴더에서 `npm install` 후 `npm run tauri build`

> **Windows 검증 상태**: Windows용 GPU 코드는 Windows 대상으로 타입 검사만 통과했고,
> 실제 Windows 기기에서 실행해 본 적은 없습니다. 처음 빌드하면 GPU 값이 작업 관리자의
> GPU 사용률과 비슷한지 확인해 주세요.

## 구조

| 경로 | 역할 |
|------|------|
| `src-tauri/src/snapshot.rs` | 한 번의 측정값(`Snapshot`)과 순수 계산 |
| `src-tauri/src/collector.rs` | CPU·메모리·디스크 수집 (`sysinfo`) |
| `src-tauri/src/gpu/macos.rs` | Mac GPU 사용률 (IOKit `IOAccelerator`) |
| `src-tauri/src/gpu/windows.rs` | Windows GPU 사용률 (성능 카운터 `GPU Engine`) |
| `src-tauri/src/tray.rs` | 트레이 아이콘, 메뉴, 요약 표시 |
| `src-tauri/src/lib.rs` | 앱 조립, 1초 주기 샘플러 스레드 |
| `ui/` | 위젯 화면 (프레임워크 없는 HTML/CSS/JS) |

설계와 구현 계획은 `docs/superpowers/`에 있습니다.
