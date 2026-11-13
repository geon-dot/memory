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
- 📌 버튼으로 항상 위에 고정, ✕ 버튼으로 닫기 (앱은 트레이에 남음, 아래 "메모리" 참고)
- 이미 실행 중일 때 다시 실행하면 하나 더 뜨지 않고 "이미 실행 중" 알림이 뜹니다
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

## 메모리

위젯 화면은 WebKit 웹뷰가 그리고, 웹뷰는 앱 본체 밖에 WebKit 보조 프로세스
(WebContent / GPU / Networking)를 따로 띄웁니다. 메모리 대부분은 이 보조 프로세스,
특히 GPU 프로세스가 씁니다. 그래서 위젯을 닫으면(✕, 트레이 왼쪽 클릭) 창과 웹뷰를
숨기는 대신 **해제**하고, 다시 열 때 새로 만듭니다. 웹뷰가 없어지면 보조 프로세스도
종료됩니다. 앱 본체는 트레이에 남아 메뉴 막대 요약을 계속 1초마다 갱신합니다.
창이 없는 동안에는 메뉴 막대에 필요한 CPU 전체 사용률과 메모리만 읽고, 코어별 값·GPU·디스크
수집과 화면 전송은 하지 않습니다.

📌 고정 상태는 웹뷰가 아니라 앱이 기억합니다(앱 설정 폴더의 `pinned` 파일).
웹뷰는 저장소를 남기지 않는 모드로 열어서, 위젯을 닫으면 Networking 프로세스도 함께 종료됩니다.

### 재는 법 (macOS)

```sh
scripts/measure-memory.sh --app "src-tauri/target/release/bundle/macos/System Monitor.app"
scripts/measure-memory.sh --pid <pid> --settle 60   # 60초 기다린 뒤 측정
```

- 앱 본체와 그 앱이 띄운 WebKit 보조 프로세스의 `footprint`(활동 상태 보기의 "메모리" 값)를
  프로세스별로 보여 주고 합계를 냅니다. RSS도 참고로 같이 보여 줍니다.
- 보조 프로세스는 macOS가 기록하는 "책임 프로세스"로 이 앱 것인지 가립니다. 근거는 스크립트 주석에 있습니다.
- 지금 위젯이 열려 있는지 닫혀 있는지도 함께 출력합니다. 열린 상태와 닫힌 상태에서 한 번씩 재면 됩니다.
  닫은 직후에는 GPU 프로세스가 1분 안쪽으로 정리되므로 `--settle 60`을 권합니다.
- 클릭 없이 상태를 바꾸려면 앱을 `SYSTEM_MONITOR_DIAG=1`로 띄운 뒤
  `".../Contents/MacOS/system-monitor" --hide` / `--show` / `--toggle`을 실행합니다.
  이 진단 모드에서는 창 생성·해제와 메뉴 막대 갱신이 stderr에 찍힙니다. 진단 모드가 아니면 이 인자들은 무시되고, 다시 실행하면 "이미 실행 중" 알림이 뜹니다.

### 변경 전후 (Apple Silicon Mac, macOS 26, 릴리스 빌드)

footprint 합계(앱 본체 + WebKit 보조 프로세스), 여러 번 잰 범위입니다.

| 상태 | 변경 전 | 변경 후 |
|------|---------|---------|
| 위젯 열림 | 173–174 MiB (본체 33 + GPU 119 + WebContent 16 + Networking 5) | 172–176 MiB (본체 33 + GPU 121 + WebContent 18 + Networking 5) |
| 위젯 닫힘 | 178–181 MiB (보조 프로세스 3개가 그대로 살아 있음) | **30–31 MiB** (본체만, 보조 프로세스 0개) — 약 83% 감소 |

- 닫힘 상태는 닫은 뒤 60초 이상 지나서 잰 값입니다. 닫은 직후 30초쯤은 GPU 프로세스가 정리 중이라 약 44 MiB입니다.
- 위젯이 열려 있을 때의 메모리는 웹뷰가 그대로라 거의 같습니다.
- 닫힘 상태에서 수집을 줄인 것은 메모리 차이가 측정 오차(약 1 MiB) 안이었고, 대신 본체 CPU 시간이
  2분에 2.3–2.5초에서 1.7–1.8초로 줄었습니다.
- Windows에서는 재지 않았습니다. 같은 코드로 창과 WebView2가 해제되지만, 보조 프로세스가 실제로 종료되는지는 확인하지 못했습니다.

## 구조

| 경로 | 역할 |
|------|------|
| `src-tauri/src/snapshot.rs` | 한 번의 측정값(`Snapshot`)과 순수 계산 |
| `src-tauri/src/collector.rs` | CPU·메모리·디스크 수집 (`sysinfo`) |
| `src-tauri/src/gpu/macos.rs` | Mac GPU 사용률 (IOKit `IOAccelerator`) |
| `src-tauri/src/gpu/windows.rs` | Windows GPU 사용률 (성능 카운터 `GPU Engine`) |
| `src-tauri/src/tray.rs` | 트레이 아이콘, 메뉴, 요약 표시 |
| `src-tauri/src/widget.rs` | 위젯 창 열기·닫기(닫으면 해제), 위치·📌 기억 |
| `src-tauri/src/lib.rs` | 앱 조립, 1초 주기 샘플러 스레드 |
| `ui/` | 위젯 화면 (프레임워크 없는 HTML/CSS/JS) |
| `scripts/measure-memory.sh` | 실행 중인 앱의 메모리 측정 (macOS) |

설계와 구현 계획은 `docs/superpowers/`에 있습니다.
