//! Per-program usage lists for the detail panel, and the pure calculations
//! behind them (grouping, ranking, rates). The OS reading lives in
//! `procdata.rs`.

use std::collections::HashMap;

/// Which per-program list the widget is showing, if any.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Detail {
    Off,
    Cpu,
    Gpu,
    Mem,
}

impl Detail {
    pub fn from_id(id: &str) -> Option<Detail> {
        match id {
            "off" => Some(Detail::Off),
            "cpu" => Some(Detail::Cpu),
            "gpu" => Some(Detail::Gpu),
            "mem" => Some(Detail::Mem),
            _ => None,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Detail::Off => "off",
            Detail::Cpu => "cpu",
            Detail::Gpu => "gpu",
            Detail::Mem => "mem",
        }
    }
}

/// One process's share, before grouping.
#[derive(Clone, Debug, PartialEq)]
pub struct ProcValue {
    pub name: String,
    pub value: f64,
    /// macOS memory only: the value is the resident size (RSS) because the
    /// process's footprint could not be read.
    pub rss: bool,
}

/// Processes of one name added together.
#[derive(serde::Serialize, Clone, Debug, PartialEq)]
pub struct Program {
    pub name: String,
    /// How many processes of this name were added together.
    pub count: u32,
    pub value: f64,
    /// See `ProcValue::rss`: true when any of the processes was counted by RSS.
    pub rss: bool,
}

/// Everything below the top programs, added together.
#[derive(serde::Serialize, Clone, Debug, PartialEq, Default)]
pub struct Others {
    /// Programs (names) left out of the top list.
    pub programs: u32,
    /// Processes in those programs.
    pub count: u32,
    pub value: f64,
}

/// Groups `procs` by name, ranks the programs by value (largest first, ties
/// by name), and keeps the first `top` that have a value above zero. The
/// rest, zero-valued programs included, are added up in `Others`.
pub fn top_programs(procs: &[ProcValue], top: usize) -> (Vec<Program>, Others) {
    let mut by_name: HashMap<&str, Program> = HashMap::new();
    for proc in procs {
        let value = if proc.value.is_finite() { proc.value.max(0.0) } else { 0.0 };
        let program = by_name.entry(proc.name.as_str()).or_insert_with(|| Program {
            name: proc.name.clone(),
            count: 0,
            value: 0.0,
            rss: false,
        });
        program.count += 1;
        program.value += value;
        program.rss |= proc.rss;
    }
    let mut programs: Vec<Program> = by_name.into_values().collect();
    programs.sort_by(|a, b| b.value.total_cmp(&a.value).then_with(|| a.name.cmp(&b.name)));

    let shown = programs.iter().take(top).take_while(|p| p.value > 0.0).count();
    let rest = programs.split_off(shown);
    let others = Others {
        programs: rest.len() as u32,
        count: rest.iter().map(|p| p.count).sum(),
        value: rest.iter().map(|p| p.value).fold(0.0, |a, b| a + b),
    };
    (programs, others)
}

/// How many programs a list shows before "그 밖".
pub const TOP_PROGRAMS: usize = 8;

/// The payload of the `detail` event: one per-program list.
///
/// `value` units by `kind`:
/// - `"cpu"`: percent of one core (100 = one core busy, like Activity
///   Monitor's % CPU), so 230 means 2.3 cores. Can exceed 100.
/// - `"gpu"`: percent of the whole GPU, 0..=100 per process.
/// - `"mem"`: bytes.
#[derive(serde::Serialize, Clone, Debug, PartialEq)]
pub struct DetailList {
    /// `"cpu"`, `"gpu"` or `"mem"`: which list this is. A list of another
    /// kind than the one the widget asked for last is a late one to drop.
    pub kind: &'static str,
    /// False when this machine cannot report this list at all (GPU per
    /// program on an Intel Mac, or without GPU counters on Windows). The
    /// lists are then empty and the numbers zero.
    pub available: bool,
    /// Largest first, at most `TOP_PROGRAMS`, only programs above zero.
    pub programs: Vec<Program>,
    /// Everything else added up.
    pub others: Others,
    /// Sum over every process counted (`programs` + `others`).
    pub total: f64,
    /// Processes counted.
    pub processes: u32,
    /// Processes left out because the OS let nothing about them be read
    /// without administrator rights (not counted in `processes`).
    pub unreadable: u32,
}

impl DetailList {
    pub fn build(kind: Detail, procs: &[ProcValue], unreadable: u32) -> DetailList {
        let (programs, others) = top_programs(procs, TOP_PROGRAMS);
        let total = programs.iter().map(|p| p.value).fold(others.value, |a, b| a + b);
        DetailList {
            kind: kind.id(),
            available: true,
            programs,
            others,
            total,
            processes: procs.len() as u32,
            unreadable,
        }
    }

    pub fn unavailable(kind: Detail) -> DetailList {
        DetailList {
            kind: kind.id(),
            available: false,
            programs: Vec::new(),
            others: Others::default(),
            total: 0.0,
            processes: 0,
            unreadable: 0,
        }
    }

    /// One line for the diagnostics: `name×count value | ...`.
    pub fn summary(&self) -> String {
        if !self.available {
            return format!("{}: unavailable", self.kind);
        }
        let shown = |v: f64| match self.kind {
            "mem" => format!("{:.0}MiB", v / 1024.0 / 1024.0),
            _ => format!("{v:.1}%"),
        };
        let mut parts: Vec<String> = self
            .programs
            .iter()
            .map(|p| {
                let rss = if p.rss { " (rss)" } else { "" };
                format!("{}×{} {}{rss}", p.name, p.count, shown(p.value))
            })
            .collect();
        parts.push(format!(
            "others {} programs/{} procs {}",
            self.others.programs,
            self.others.count,
            shown(self.others.value)
        ));
        format!(
            "{}: {} | total {} over {} procs, {} unreadable",
            self.kind,
            parts.join(" | "),
            shown(self.total),
            self.processes,
            self.unreadable
        )
    }
}

/// CPU time of one process as `ps` prints it (`time` column), in
/// hundredths of a second. Accepts `S.cc`, `M:SS.cc`, `H:MM:SS.cc` and a
/// leading `D-`.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))] // read through `ps` on macOS
pub fn parse_cpu_time(text: &str) -> Option<u64> {
    let (days, clock) = match text.split_once('-') {
        Some((d, rest)) => (d.parse::<u64>().ok()?, rest),
        None => (0, text),
    };
    let fields: Vec<&str> = clock.split(':').collect();
    let (last, upper) = fields.split_last()?;
    if upper.len() > 2 {
        return None;
    }
    let (secs, hundredths) = match last.split_once('.') {
        Some((s, f)) => {
            if f.is_empty() || f.len() > 2 {
                return None;
            }
            let frac: u64 = f.parse().ok()?;
            // "5" is half a second, "25" a quarter.
            (s.parse::<u64>().ok()?, if f.len() == 1 { frac * 10 } else { frac })
        }
        None => (last.parse::<u64>().ok()?, 0),
    };
    // `upper` is [minutes] or [hours, minutes].
    let mut minutes = days * 24 * 60;
    let mut upper_minutes = 0u64;
    for field in upper {
        upper_minutes = upper_minutes * 60 + field.parse::<u64>().ok()?;
    }
    minutes += upper_minutes;
    Some((minutes * 60 + secs) * 100 + hundredths)
}

/// One line of `ps -axo pid=,time=,rss=,comm=`.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))] // read through `ps` on macOS
pub struct PsRow {
    pub pid: u32,
    /// Total CPU time so far, hundredths of a second.
    pub cpu_cs: u64,
    pub rss_bytes: u64,
    /// The executable's file name (the last part of `comm`).
    pub name: String,
}

/// Parses `ps -axo pid=,time=,rss=,comm=` output; lines that do not parse
/// are skipped. `rss` is in KiB there.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))] // read through `ps` on macOS
pub fn parse_ps(output: &str) -> Vec<PsRow> {
    output.lines().filter_map(parse_ps_line).collect()
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))] // read through `ps` on macOS
fn parse_ps_line(line: &str) -> Option<PsRow> {
    // Columns are separated by runs of spaces; the command itself may hold
    // spaces, so it is everything after the third column.
    let (pid, rest) = line.trim_start().split_once(char::is_whitespace)?;
    let (time, rest) = rest.trim_start().split_once(char::is_whitespace)?;
    let (rss, comm) = rest.trim_start().split_once(char::is_whitespace)?;
    let comm = comm.trim();
    let name = comm.rsplit('/').next().unwrap_or(comm);
    if name.is_empty() {
        return None;
    }
    Some(PsRow {
        pid: pid.parse().ok()?,
        cpu_cs: parse_cpu_time(time)?,
        rss_bytes: rss.parse::<u64>().ok()?.saturating_mul(1024),
        name: name.to_string(),
    })
}

/// Percent of one core (100 = one core busy the whole time) each process
/// used between two readings of total CPU time (hundredths of a second),
/// `elapsed_secs` apart. A process missing from `before` started in between,
/// so all of its time counts. A counter that went down (pid reused) counts
/// from zero.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))] // read through `ps` on macOS
pub fn cpu_percent(before: Option<u64>, now: u64, elapsed_secs: f64) -> f64 {
    if !(elapsed_secs > 0.0) {
        return 0.0;
    }
    let used = match before {
        Some(b) if b <= now => now - b,
        _ => now,
    };
    used as f64 / 100.0 / elapsed_secs * 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(name: &str, value: f64) -> ProcValue {
        ProcValue { name: name.into(), value, rss: false }
    }

    #[test]
    fn detail_ids_round_trip() {
        for d in [Detail::Off, Detail::Cpu, Detail::Gpu, Detail::Mem] {
            assert_eq!(Detail::from_id(d.id()), Some(d));
        }
        assert_eq!(Detail::from_id("cpu"), Some(Detail::Cpu));
        assert_eq!(Detail::from_id("disk"), None);
    }

    #[test]
    fn same_name_is_one_program_with_a_count() {
        let procs = vec![
            p("Google Chrome Helper", 10.0),
            p("WindowServer", 25.0),
            p("Google Chrome Helper", 20.0),
            p("Google Chrome Helper", 5.0),
        ];
        let (top, others) = top_programs(&procs, 8);
        assert_eq!(
            top,
            vec![
                Program { name: "Google Chrome Helper".into(), count: 3, value: 35.0, rss: false },
                Program { name: "WindowServer".into(), count: 1, value: 25.0, rss: false },
            ]
        );
        assert_eq!(others, Others::default());
    }

    #[test]
    fn keeps_the_top_n_and_adds_up_the_rest() {
        let procs: Vec<ProcValue> =
            (1..=10).map(|i| p(&format!("p{i:02}"), i as f64)).collect();
        let (top, others) = top_programs(&procs, 8);
        let names: Vec<&str> = top.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["p10", "p09", "p08", "p07", "p06", "p05", "p04", "p03"]);
        assert_eq!(others, Others { programs: 2, count: 2, value: 3.0 });
    }

    #[test]
    fn zero_valued_programs_go_to_others() {
        let procs = vec![p("busy", 3.0), p("idle", 0.0), p("idle", 0.0), p("nap", 0.0)];
        let (top, others) = top_programs(&procs, 8);
        assert_eq!(top.len(), 1);
        assert_eq!(others, Others { programs: 2, count: 3, value: 0.0 });
    }

    #[test]
    fn ties_rank_by_name() {
        let (top, _) = top_programs(&[p("b", 1.0), p("a", 1.0)], 8);
        assert_eq!(top[0].name, "a");
    }

    #[test]
    fn rss_marks_the_program() {
        let procs = vec![
            ProcValue { name: "x".into(), value: 1.0, rss: false },
            ProcValue { name: "x".into(), value: 2.0, rss: true },
        ];
        assert!(top_programs(&procs, 8).0[0].rss);
    }

    #[test]
    fn non_finite_values_count_as_zero() {
        let (top, others) = top_programs(&[p("x", f64::NAN), p("y", -1.0)], 8);
        assert!(top.is_empty());
        assert_eq!(others, Others { programs: 2, count: 2, value: 0.0 });
    }

    #[test]
    fn empty_is_empty() {
        assert_eq!(top_programs(&[], 8), (vec![], Others::default()));
    }

    #[test]
    fn detail_list_totals_everything_and_counts_processes() {
        let procs: Vec<ProcValue> =
            (1..=10).map(|i| p(&format!("p{i:02}"), i as f64)).collect();
        let list = DetailList::build(Detail::Cpu, &procs, 4);
        assert_eq!(list.kind, "cpu");
        assert!(list.available);
        assert_eq!(list.programs.len(), TOP_PROGRAMS);
        assert_eq!(list.total, 55.0);
        assert_eq!(list.processes, 10);
        assert_eq!(list.unreadable, 4);
    }

    #[test]
    fn unavailable_list_is_empty_and_serializes_flat() {
        let list = DetailList::unavailable(Detail::Gpu);
        assert_eq!(list.summary(), "gpu: unavailable");
        let json = serde_json::to_value(&list).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "kind": "gpu", "available": false, "programs": [],
                "others": {"programs": 0, "count": 0, "value": 0.0},
                "total": 0.0, "processes": 0, "unreadable": 0
            })
        );
    }

    #[test]
    fn summary_shows_memory_in_mib() {
        let procs = vec![ProcValue { name: "x".into(), value: 2.0 * 1024.0 * 1024.0, rss: true }];
        let s = DetailList::build(Detail::Mem, &procs, 0).summary();
        assert!(s.starts_with("mem: x×1 2MiB (rss) | others 0 programs/0 procs 0MiB"), "{s}");
    }

    #[test]
    fn parses_ps_cpu_times() {
        assert_eq!(parse_cpu_time("0:00.00"), Some(0));
        assert_eq!(parse_cpu_time("3:46.80"), Some(22_680));
        assert_eq!(parse_cpu_time("115:39.25"), Some(693_925));
        assert_eq!(parse_cpu_time("1:02:03.04"), Some(372_304));
        assert_eq!(parse_cpu_time("2-01:00:00.00"), Some(2 * 8_640_000 + 360_000));
        assert_eq!(parse_cpu_time("12.5"), Some(1_250));
        assert_eq!(parse_cpu_time(""), None);
        assert_eq!(parse_cpu_time("x:00.00"), None);
    }

    #[test]
    fn parses_ps_output() {
        let out = "  176 115:39.25  55408 /System/Library/PrivateFrameworks/SkyLight.framework/Resources/WindowServer\n\
                   22601   1:02.00 391992 /Applications/KakaoTalk.app/Contents/MacOS/KakaoTalk\n\
                   17000   0:00.10   1024 /Applications/Google Chrome.app/Contents/Frameworks/x/Google Chrome Helper (Renderer).app/Contents/MacOS/Google Chrome Helper (Renderer)\n\
                   99   0:00.00      0 autofsd\n\
                   garbage line\n";
        let rows = parse_ps(out);
        assert_eq!(rows.len(), 4);
        assert_eq!(
            rows[0],
            PsRow { pid: 176, cpu_cs: 693_925, rss_bytes: 55_408 * 1024, name: "WindowServer".into() }
        );
        assert_eq!(rows[2].name, "Google Chrome Helper (Renderer)");
        assert_eq!(rows[3].name, "autofsd");
    }

    #[test]
    fn cpu_percent_is_per_core() {
        // 230 hundredths of CPU time in one second: 2.3 cores.
        assert!((cpu_percent(Some(1_000), 1_230, 1.0) - 230.0).abs() < 1e-9);
        assert!((cpu_percent(Some(0), 100, 2.0) - 50.0).abs() < 1e-9);
        // Started in between: all of its time counts.
        assert!((cpu_percent(None, 40, 1.0) - 40.0).abs() < 1e-9);
        // Pid reused by a younger process: count from zero.
        assert!((cpu_percent(Some(500), 20, 1.0) - 20.0).abs() < 1e-9);
        assert_eq!(cpu_percent(Some(1), 2, 0.0), 0.0);
    }
}
