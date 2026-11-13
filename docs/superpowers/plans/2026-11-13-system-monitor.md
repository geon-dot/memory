# System Monitor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Mac과 Windows에서 CPU·GPU·메모리·디스크 상태를 1초 주기로 보여주는 트레이/메뉴 막대 위젯 앱을 만든다.

**Architecture:** Rust 백엔드의 샘플러 스레드가 1초마다 `Snapshot`을 만들어 Tauri 이벤트 `snapshot`으로 내보낸다. 번들러 없는 정적 HTML/CSS/JS 프런트엔드가 이벤트를 받아 행 4개(CPU, GPU, MEM, DISK)를 그린다. GPU만 OS별 모듈(`gpu/macos.rs`, `gpu/windows.rs`)로 갈라진다.

**Tech Stack:** Rust (stable), Tauri 2 (`tray-icon` feature), `sysinfo`, macOS: `io-kit-sys` + `core-foundation`, Windows: `windows` 크레이트(PDH), 프런트엔드: 프레임워크 없는 ES 모듈, 테스트: `cargo test`, `node --test`.

**Spec:** `docs/superpowers/specs/2026-11-13-system-monitor-design.md`

## Global Constraints

- 갱신 주기 1초 (1000ms).
- 관리자 권한을 요구하지 않는다.
- 프런트엔드에 프레임워크·번들러를 쓰지 않는다 (`withGlobalTauri: true`, `frontendDist: "../ui"`).
- GPU 값을 읽지 못하면 `None` → 화면에 `N/A`. 패닉 금지.
- 첫 버전 제외: 프로세스 목록, 온도/팬, 알림, 기록 저장, 조각 모음.
- 창을 닫아도 앱은 트레이에 남는다. 종료는 트레이 메뉴 `종료`로만.
- macOS에서는 Dock 아이콘 없이 메뉴 막대에만 표시 (`ActivationPolicy::Accessory`).
- Windows 전용 코드는 `#[cfg(target_os = "windows")]`, macOS 전용은 `#[cfg(target_os = "macos")]`. 그 외 OS는 GPU `None`.

## Review Focus

1. GPU가 없거나 통계를 읽을 수 없는 기기 → GPU 행이 `N/A`, 나머지는 정상 동작. (Task 3, 4, 6)
2. 총량이 0으로 보고되는 경우(메모리/디스크) → 0으로 나누지 않고 0% 또는 `N/A`. (Task 2, 6)
3. 시스템 디스크를 찾지 못하는 경우 → `disk_total = 0`, 화면 `N/A`. (Task 2, 6)
4. GPU가 여러 개이거나 Windows에서 엔진 인스턴스가 수백 개 → 엔진 종류별 합의 최댓값, 0–100으로 제한. (Task 4)
5. 코어가 매우 많은 기기(64코어 등) → 코어 막대가 줄바꿈되어 창을 넘치지 않음. (Task 6)

## File Structure

```
package.json                     # @tauri-apps/cli, 스크립트, "type": "module"
ui/index.html                    # 위젯 마크업
ui/style.css                     # 스타일 (라이트/다크)
ui/format.js                     # 순수 함수: 포맷, 히스토리, 스파크라인
ui/format.test.js                # node --test
ui/main.js                       # 이벤트 수신 → DOM 갱신, 핀 버튼
src-tauri/Cargo.toml
src-tauri/build.rs
src-tauri/tauri.conf.json
src-tauri/capabilities/default.json
src-tauri/icons/                 # `tauri icon`으로 생성
src-tauri/src/main.rs            # system_monitor_lib::run() 호출만
src-tauri/src/lib.rs             # Tauri 빌더, 샘플러 스레드, 명령
src-tauri/src/snapshot.rs        # Snapshot 타입 + 순수 계산
src-tauri/src/collector.rs       # sysinfo 래퍼 → Snapshot
src-tauri/src/gpu/mod.rs         # GpuSampler (OS별 선택)
src-tauri/src/gpu/macos.rs
src-tauri/src/gpu/windows.rs     # PDH + aggregate_engines
src-tauri/src/tray.rs            # 트레이 아이콘, 메뉴, 제목
```

---

### Task 1: 도구 설치와 Tauri 뼈대

**Files:**
- Create: `package.json`, `.gitignore`, `ui/index.html`, `src-tauri/Cargo.toml`, `src-tauri/build.rs`, `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`, `src-tauri/src/main.rs`, `src-tauri/src/lib.rs`, `src-tauri/icons/*`

**Interfaces:**
- Produces: 크레이트 이름 `system-monitor`, 라이브러리 이름 `system_monitor_lib`, `pub fn run()` in `lib.rs`. 창 라벨 `main`. npm 스크립트 `npm run tauri`, `npm test` (`node --test ui/`).

- [ ] **Step 1: Rust 설치** (이 기기에 없음)

Run: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && source "$HOME/.cargo/env" && rustc --version`
Expected: `rustc 1.x.y` 출력.

- [ ] **Step 2: `package.json` 작성 후 `npm install`**

`"type": "module"`, devDependency `@tauri-apps/cli` (^2), scripts: `"tauri": "tauri"`, `"test": "node --test ui/"`.

- [ ] **Step 3: `src-tauri/` 뼈대 작성**

`tauri.conf.json` 고정 값:
- `productName`: `System Monitor`, `identifier`: `com.geon.systemmonitor`
- `build.frontendDist`: `../ui`, `app.withGlobalTauri`: `true`
- 창: `label: "main"`, `width: 300`, `height: 240`, `resizable: false`, `decorations: false`, `transparent: false`, `skipTaskbar: true`, `title: "System Monitor"`

`Cargo.toml` 의존성: `tauri = { version = "2", features = ["tray-icon"] }`, `tauri-build = "2"` (build), `serde = { version = "1", features = ["derive"] }`, `sysinfo`(최신 안정). `[lib] name = "system_monitor_lib"`, `crate-type = ["staticlib", "cdylib", "rlib"]`.
대상별: `[target.'cfg(target_os = "macos")'.dependencies]` `io-kit-sys`, `core-foundation`; `[target.'cfg(target_os = "windows")'.dependencies]` `windows = { features = ["Win32_System_Performance", "Win32_Foundation"] }`.

`capabilities/default.json`: 창 `main`에 `core:default`, `core:event:default`, `core:window:allow-start-dragging`.

`ui/index.html`: `<h1>System Monitor</h1>`만 있는 자리표시자.

- [ ] **Step 4: 아이콘 생성**

단색 원형 1024×1024 PNG를 스크래치패드에 만들어 `npm run tauri icon <png>` 실행.
Expected: `src-tauri/icons/`에 `icon.icns`, `icon.ico`, `32x32.png` 등 생성.

- [ ] **Step 5: 빌드 확인**

Run: `cd src-tauri && cargo build`
Expected: 오류 없이 완료.

- [ ] **Step 6: Commit** — `chore: scaffold tauri app`

---

### Task 2: Snapshot 타입과 순수 계산

**Files:**
- Create: `src-tauri/src/snapshot.rs`
- Modify: `src-tauri/src/lib.rs` (`mod snapshot;`)

**Interfaces:**
- Produces:

```rust
#[derive(serde::Serialize, Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub cpu_total: f32,        // 0.0..=100.0
    pub cpu_cores: Vec<f32>,   // 각 0.0..=100.0
    pub gpu: Option<f32>,      // 0.0..=100.0
    pub mem_used: u64,         // bytes
    pub mem_total: u64,
    pub disk_used: u64,        // bytes, 시스템 디스크
    pub disk_total: u64,       // 0이면 "찾지 못함"
}

pub struct DiskInfo { pub mount: String, pub total: u64, pub available: u64 }

pub fn clamp_pct(v: f32) -> f32;                 // NaN → 0.0, 그 외 0..=100
pub fn mean(values: &[f32]) -> f32;              // 빈 슬라이스 → 0.0
pub fn pick_system_disk(disks: &[DiskInfo], root: &str) -> (u64, u64); // (used, total); 없으면 (0, 0)
pub fn tray_title(s: &Snapshot) -> String;       // "CPU 37%  MEM 58%"
```

- [ ] **Step 1: 실패하는 테스트 작성** (`snapshot.rs` 하단 `#[cfg(test)] mod tests`)

```rust
#[test] fn clamp_handles_nan_and_range() {
    assert_eq!(clamp_pct(f32::NAN), 0.0);
    assert_eq!(clamp_pct(-3.0), 0.0);
    assert_eq!(clamp_pct(140.0), 100.0);
    assert_eq!(clamp_pct(42.5), 42.5);
}
#[test] fn mean_of_empty_is_zero() { assert_eq!(mean(&[]), 0.0); assert_eq!(mean(&[10.0, 30.0]), 20.0); }
#[test] fn picks_disk_by_mount_case_insensitive() {
    let d = vec![
        DiskInfo { mount: "/System/Volumes/Data".into(), total: 500, available: 100 },
        DiskInfo { mount: "/".into(), total: 500, available: 188 },
    ];
    assert_eq!(pick_system_disk(&d, "/"), (312, 500));
    let w = vec![DiskInfo { mount: "C:\\".into(), total: 1000, available: 400 }];
    assert_eq!(pick_system_disk(&w, "c:\\"), (600, 1000));
}
#[test] fn missing_disk_is_zero() { assert_eq!(pick_system_disk(&[], "/"), (0, 0)); }
#[test] fn available_larger_than_total_does_not_underflow() {
    let d = vec![DiskInfo { mount: "/".into(), total: 10, available: 20 }];
    assert_eq!(pick_system_disk(&d, "/"), (0, 10));
}
#[test] fn tray_title_formats_and_survives_zero_total() {
    let mut s = Snapshot { cpu_total: 36.6, cpu_cores: vec![], gpu: None,
        mem_used: 58, mem_total: 100, disk_used: 0, disk_total: 0 };
    assert_eq!(tray_title(&s), "CPU 37%  MEM 58%");
    s.mem_total = 0;
    assert_eq!(tray_title(&s), "CPU 37%  MEM 0%");
}
```

- [ ] **Step 2: 실패 확인** — Run: `cargo test snapshot` → 컴파일 오류(함수 없음)
- [ ] **Step 3: 구현** — `pick_system_disk`는 `saturating_sub` 사용, 마운트 비교는 `eq_ignore_ascii_case`.
- [ ] **Step 4: 통과 확인** — Run: `cargo test snapshot` → 6 passed
- [ ] **Step 5: Commit** — `feat: snapshot types and pure calculations`

---

### Task 3: macOS GPU 샘플러와 GpuSampler 진입점

**Files:**
- Create: `src-tauri/src/gpu/mod.rs`, `src-tauri/src/gpu/macos.rs`
- Modify: `src-tauri/src/lib.rs` (`mod gpu;`)

**Interfaces:**
- Produces:

```rust
// gpu/mod.rs
pub struct GpuSampler { /* OS별 내부 상태 */ }
impl GpuSampler {
    pub fn new() -> Self;                      // 실패해도 패닉 없이 생성
    pub fn sample(&mut self) -> Option<f32>;   // 0.0..=100.0 또는 None
}
// gpu/macos.rs
pub fn sample() -> Option<f32>;
```

`mod.rs`는 `cfg`로 macOS → `macos::sample()`, Windows → Task 4의 `windows::PdhGpu`, 그 외 → `None`.

- [ ] **Step 1: 실패하는 테스트 작성** (`gpu/mod.rs`)

```rust
#[test] fn sample_is_none_or_in_range() {
    let mut g = GpuSampler::new();
    for _ in 0..3 {
        if let Some(v) = g.sample() { assert!((0.0..=100.0).contains(&v), "got {v}"); }
    }
}
```

- [ ] **Step 2: 실패 확인** — Run: `cargo test gpu` → 컴파일 오류
- [ ] **Step 3: `macos::sample()` 구현**

`IOServiceMatching("IOAccelerator")` → `IOServiceGetMatchingServices` → 각 항목에 `IORegistryEntryCreateCFProperties` → `PerformanceStatistics` 사전의 `Device Utilization %`(정수) 읽기. 여러 가속기면 최댓값. 키가 없거나 어떤 IOKit 호출이든 실패하면 `None`. 모든 `io_object_t`는 `IOObjectRelease`, CF 객체는 `core-foundation` 래퍼(`wrap_under_create_rule`)로 해제. 결과는 `clamp_pct`.

- [ ] **Step 4: 통과 확인** — Run: `cargo test gpu` → PASS
- [ ] **Step 5: 실제 값 확인**

임시 `#[test] #[ignore] fn print_gpu()`로 값을 출력: `cargo test print_gpu -- --ignored --nocapture`
Expected: Apple Silicon에서 `Some(n)` 출력. `None`이면 `ioreg -r -d 1 -c IOAccelerator | grep -i utilization`으로 키 이름을 확인해 수정.

- [ ] **Step 6: Commit** — `feat: macOS GPU utilization via IOKit`

---

### Task 4: Windows GPU 샘플러

**Files:**
- Create: `src-tauri/src/gpu/windows.rs`
- Modify: `src-tauri/src/gpu/mod.rs`

**Interfaces:**
- Consumes: `snapshot::clamp_pct`
- Produces:

```rust
// cfg 없음 — 모든 OS에서 컴파일·테스트
pub fn aggregate_engines(samples: &[(String, f64)]) -> Option<f32>;

#[cfg(target_os = "windows")]
pub struct PdhGpu { /* query, counter 핸들 */ }
#[cfg(target_os = "windows")]
impl PdhGpu {
    pub fn new() -> Option<Self>;
    pub fn sample(&mut self) -> Option<f32>;
}
// Drop에서 PdhCloseQuery
```

`aggregate_engines` 규칙(작업 관리자와 동일): 인스턴스 이름에서 `engtype_` 뒤 문자열로 묶어 합산하고, 묶음 합의 최댓값을 `clamp_pct`. `engtype_`이 없는 이름은 무시. 입력이 비었거나 전부 무시되면 `None`.

- [ ] **Step 1: 실패하는 테스트 작성**

```rust
#[test] fn sums_per_engine_type_and_takes_max() {
    let s = vec![
        ("pid_1_luid_0x0_0x1_phys_0_eng_0_engtype_3D".to_string(), 20.0),
        ("pid_2_luid_0x0_0x1_phys_0_eng_0_engtype_3D".to_string(), 15.5),
        ("pid_1_luid_0x0_0x1_phys_0_eng_1_engtype_VideoDecode".to_string(), 30.0),
    ];
    assert_eq!(aggregate_engines(&s), Some(35.5));
}
#[test] fn clamps_over_100() {
    let s = vec![("a_engtype_3D".to_string(), 80.0), ("b_engtype_3D".to_string(), 70.0)];
    assert_eq!(aggregate_engines(&s), Some(100.0));
}
#[test] fn empty_or_unparseable_is_none() {
    assert_eq!(aggregate_engines(&[]), None);
    assert_eq!(aggregate_engines(&[("garbage".to_string(), 50.0)]), None);
}
```

- [ ] **Step 2: 실패 확인** — Run: `cargo test aggregate` → 컴파일 오류
- [ ] **Step 3: `aggregate_engines` 구현**
- [ ] **Step 4: 통과 확인** — Run: `cargo test aggregate` → 3 passed
- [ ] **Step 5: `PdhGpu` 구현**

`PdhOpenQueryW` → `PdhAddEnglishCounterW(r"\GPU Engine(*)\Utilization Percentage")` → `new()`에서 `PdhCollectQueryData` 1회(기준 샘플). `sample()`: `PdhCollectQueryData` → `PdhGetFormattedCounterArrayW`(`PDH_FMT_DOUBLE`, 먼저 크기 조회 후 버퍼 할당) → `(이름, 값)` 목록 → `aggregate_engines`. 어떤 PDH 호출이든 실패하면 `None`. `mod.rs`의 `GpuSampler`는 `Option<PdhGpu>`를 들고, `None`이면 항상 `None`을 돌려준다.

- [ ] **Step 6: Windows 대상 타입 검사 (최선 노력)**

Run: `rustup target add x86_64-pc-windows-msvc && cargo check --lib --target x86_64-pc-windows-msvc`
Expected: 오류 없음. `tauri-build`의 리소스 컴파일 등 호스트 도구 문제로 실패하면(코드 오류가 아니면) 그 사실을 기록하고 넘어간다 — 최종 보고에 "Windows 빌드 미검증"으로 명시.

- [ ] **Step 7: Commit** — `feat: Windows GPU utilization via PDH`

---

### Task 5: 수집기와 샘플러 스레드

**Files:**
- Create: `src-tauri/src/collector.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `Snapshot`, `DiskInfo`, `clamp_pct`, `mean`, `pick_system_disk`, `GpuSampler`
- Produces:

```rust
pub struct Collector { /* sysinfo::System, Disks, GpuSampler */ }
impl Collector {
    pub fn new() -> Self;
    pub fn sample(&mut self) -> Snapshot;
}
pub fn system_root() -> String; // Windows: 환경변수 SystemDrive + "\\" (기본 "C:\\"), 그 외 "/"
```

`lib.rs`: `setup`에서 스레드를 띄워 `Collector::new()` 후 루프 — `sample()` → `app.emit("snapshot", &snap)` → `tray::update_title`(Task 7에서 연결) → `sleep(1000ms)`. Tauri 명령 `#[tauri::command] fn set_pinned(window: tauri::WebviewWindow, pinned: bool)` → `window.set_always_on_top(pinned)`.

- [ ] **Step 1: 실패하는 테스트 작성** (`collector.rs`)

```rust
#[test] fn sample_is_sane_on_this_machine() {
    let mut c = Collector::new();
    std::thread::sleep(std::time::Duration::from_millis(250));
    let s = c.sample();
    assert!(!s.cpu_cores.is_empty());
    assert!((0.0..=100.0).contains(&s.cpu_total));
    assert!(s.cpu_cores.iter().all(|v| (0.0..=100.0).contains(v)));
    assert!(s.mem_total > 0 && s.mem_used <= s.mem_total);
    assert!(s.disk_total > 0 && s.disk_used <= s.disk_total);
}
```

- [ ] **Step 2: 실패 확인** — Run: `cargo test collector` → 컴파일 오류
- [ ] **Step 3: `Collector` 구현**

`sysinfo`로 CPU·메모리만 갱신(`refresh_cpu_usage`, `refresh_memory`) — 프로세스 목록은 갱신하지 않는다(가벼움 유지). 디스크 목록은 매 샘플마다가 아니라 5회에 1회 갱신. `cpu_total = mean(cores)`.

- [ ] **Step 4: 통과 확인** — Run: `cargo test collector` → PASS
- [ ] **Step 5: 스레드와 `set_pinned` 연결, `cargo build` 확인**
- [ ] **Step 6: Commit** — `feat: collector and 1s sampler thread`

---

### Task 6: 위젯 화면

**Files:**
- Create: `ui/format.js`, `ui/format.test.js`, `ui/main.js`, `ui/style.css`
- Modify: `ui/index.html`

**Interfaces:**
- Consumes: 이벤트 `snapshot` (payload = `Snapshot`, 필드명 snake_case 그대로), 명령 `set_pinned({ pinned })`. `window.__TAURI__.event.listen`, `window.__TAURI__.core.invoke`.
- Produces (`format.js`, ES 모듈):

```js
export function formatBytes(n)            // 1024 기준, "9.2 GB" / "512 MB"; GB 이상 소수 1자리
export function percent(used, total)      // 정수 0..100; total <= 0 → 0
export function pushHistory(arr, v, max = 60) // 새 배열 반환, 길이 max 초과분은 앞에서 버림
export function sparklinePoints(arr, w, h)    // SVG polyline points 문자열; 값 0→y=h, 100→y=0
export function gpuLabel(v)               // null/undefined → "N/A", 그 외 "12%"
export function usageLabel(used, total)   // total <= 0 → "N/A", 그 외 "9.2 / 16.0 GB"
```

- [ ] **Step 1: 실패하는 테스트 작성** (`ui/format.test.js`, `node:test` + `node:assert/strict`)

```js
test('formatBytes', () => {
  assert.equal(formatBytes(0), '0 MB');
  assert.equal(formatBytes(512 * 1024 ** 2), '512 MB');
  assert.equal(formatBytes(9.2 * 1024 ** 3), '9.2 GB');
});
test('percent guards zero total', () => {
  assert.equal(percent(5, 0), 0);
  assert.equal(percent(58, 100), 58);
  assert.equal(percent(200, 100), 100);
});
test('pushHistory caps length and does not mutate', () => {
  const a = [1, 2, 3];
  const b = pushHistory(a, 4, 3);
  assert.deepEqual(b, [2, 3, 4]);
  assert.deepEqual(a, [1, 2, 3]);
});
test('sparklinePoints maps range', () => {
  assert.equal(sparklinePoints([0, 100], 10, 20), '0,20 10,0');
  assert.equal(sparklinePoints([], 10, 20), '');
  assert.equal(sparklinePoints([50], 10, 20), '0,10');
});
test('labels', () => {
  assert.equal(gpuLabel(null), 'N/A');
  assert.equal(gpuLabel(12.4), '12%');
  assert.equal(usageLabel(0, 0), 'N/A');
  assert.equal(usageLabel(9.2 * 1024 ** 3, 16 * 1024 ** 3), '9.2 / 16.0 GB');
});
```

- [ ] **Step 2: 실패 확인** — Run: `npm test` → FAIL (모듈 없음)
- [ ] **Step 3: `format.js` 구현**
- [ ] **Step 4: 통과 확인** — Run: `npm test` → 5 pass
- [ ] **Step 5: `index.html`, `style.css`, `main.js` 작성**

구성:
- 상단 바: 제목 `System Monitor`(`data-tauri-drag-region`으로 창 이동), 핀 버튼(`aria-pressed` 토글 → `set_pinned`).
- 행 4개: `CPU`(퍼센트 + 60초 스파크라인 SVG, 아래에 코어별 얇은 막대 — `flex-wrap`으로 줄바꿈), `GPU`(퍼센트 + 스파크라인, `null`이면 `N/A`이고 스파크라인 숨김), `MEM`(`usageLabel` + 막대), `DISK`(`usageLabel` + 막대).
- `prefers-color-scheme`으로 라이트/다크. 글꼴은 시스템 UI, 숫자는 `font-variant-numeric: tabular-nums`.
- `window.__TAURI__`가 없으면(브라우저로 직접 열었을 때) 오류 없이 대기 상태 표시.

- [ ] **Step 6: Commit** — `feat: widget UI`

---

### Task 7: 트레이 / 메뉴 막대

**Files:**
- Create: `src-tauri/src/tray.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `snapshot::tray_title`, 창 라벨 `main`
- Produces:

```rust
pub const TRAY_ID: &str = "main";
pub fn build(app: &tauri::AppHandle) -> tauri::Result<()>;
pub fn update_title(app: &tauri::AppHandle, s: &Snapshot); // macOS: set_title, 전 OS: set_tooltip
```

동작:
- 메뉴 항목 `열기`(창 표시 + 포커스), `종료`(`app.exit(0)`).
- 트레이 아이콘 왼쪽 클릭 → 창 표시/숨김 토글 (`show_menu_on_left_click(false)`).
- `lib.rs`: 창 `CloseRequested` → `api.prevent_close()` 후 `hide()`.
- `lib.rs`: macOS에서 `app.set_activation_policy(ActivationPolicy::Accessory)`.
- 샘플러 스레드에서 매 샘플 `update_title` 호출.

- [ ] **Step 1: 구현** (순수 로직 `tray_title`은 Task 2에서 테스트됨)
- [ ] **Step 2: 전체 테스트** — Run: `cargo test && npm test` → 모두 PASS
- [ ] **Step 3: 실행 확인**

Run: `npm run tauri dev`
확인 항목: 메뉴 막대에 `CPU n%  MEM n%`가 1초마다 바뀜 / 아이콘 클릭으로 창 토글 / 4개 행 값이 활성 상태 보기(Activity Monitor)와 대략 일치 / 핀 버튼으로 항상 위 / 창을 닫아도 메뉴 막대에 남음 / `종료`로 프로세스 종료 / Dock 아이콘 없음.

- [ ] **Step 4: 자체 자원 사용 확인**

Run: `ps -o rss,%cpu,comm -p $(pgrep -f system-monitor)` (dev 실행 중)
Expected: 평균 CPU 수 % 이하. 눈에 띄게 높으면 디스크 갱신 주기와 sysinfo 갱신 범위를 재확인.

- [ ] **Step 5: Commit** — `feat: tray icon, menu bar title, hide-on-close`

---

### Task 8: 배포 빌드와 README

**Files:**
- Create: `README.md`

- [ ] **Step 1: macOS 번들 빌드** — Run: `npm run tauri build` → `src-tauri/target/release/bundle/macos/System Monitor.app` 생성, 실행해 Task 7 Step 3 항목 재확인.
- [ ] **Step 2: `README.md` 작성** — 기능, 개발 실행(`npm install`, `npm run tauri dev`), 빌드, **Windows에서 빌드하는 법**(Rust + MSVC Build Tools + WebView2, `npm run tauri build`), Windows GPU 값은 실제 Windows 기기에서 미검증이라는 점, 첫 버전 제외 항목.
- [ ] **Step 3: Commit** — `docs: README and build instructions`
