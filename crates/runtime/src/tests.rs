#![allow(clippy::unwrap_used)]

use super::*;
#[cfg_attr(windows, allow(unused_imports))]
use mango_core::Session;
#[cfg_attr(windows, allow(unused_imports))]
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[cfg_attr(windows, allow(dead_code))]
fn collect() -> (EventCb, Arc<Mutex<Vec<GameEvent>>>) {
    let store = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&store);
    let cb: EventCb = Arc::new(move |e| sink.lock().unwrap().push(e));
    (cb, store)
}

#[cfg_attr(windows, allow(dead_code))]
fn lines(store: &Arc<Mutex<Vec<GameEvent>>>) -> Vec<(String, LogLevel)> {
    store
        .lock()
        .unwrap()
        .iter()
        .filter_map(|e| match e {
            GameEvent::Log { line, level } => Some((line.clone(), *level)),
            _ => None,
        })
        .collect()
}

fn event_xml(level: &str, thread: &str, msg: &str) -> String {
    format!(
        "<log4j:Event logger=\"x\" timestamp=\"1700000000000\" level=\"{level}\" thread=\"{thread}\"><log4j:Message><![CDATA[{msg}]]></log4j:Message></log4j:Event>"
    )
}

fn run_parser(chunks: &[&str]) -> Vec<(String, LogLevel)> {
    let mut p = LogParser::new();
    let mut out = Vec::new();
    for c in chunks {
        p.feed(c, &mut |l, lv| out.push((l.to_string(), lv)));
    }
    p.finish(&mut |l, lv| out.push((l.to_string(), lv)));
    out
}

#[test]
fn parser_formats_events_split_across_chunks() {
    let xml = event_xml("INFO", "Render thread", "Hello");
    let (a, b) = xml.split_at(37);
    let out = run_parser(&["JVM warning\n", a, b, "\n"]);
    assert_eq!(out[0], ("JVM warning".to_string(), LogLevel::Info));
    let (line, level) = &out[1];
    assert_eq!(*level, LogLevel::Info);
    assert!(line.ends_with("] [Render thread/INFO]: Hello"), "{line}");
    assert_eq!(line.as_bytes()[0], b'[');
    assert_eq!(line.as_bytes()[9], b']');
    assert_eq!(out.len(), 2);
}

#[test]
fn parser_error_events_and_throwables() {
    let xml = "<log4j:Event logger=\"x\" timestamp=\"1700000000000\" level=\"ERROR\" thread=\"main\"><log4j:Message><![CDATA[boom]]></log4j:Message><log4j:Throwable><![CDATA[java.lang.Oops\n\tat a.B(C.java)\n]]></log4j:Throwable></log4j:Event>";
    let out = run_parser(&[xml]);
    assert_eq!(out.len(), 3);
    assert!(out.iter().all(|(_, l)| *l == LogLevel::Error));
    assert!(out[0].0.ends_with("[main/ERROR]: boom"));
    assert_eq!(out[1].0, "java.lang.Oops");
    assert_eq!(out[2].0, "\tat a.B(C.java)");
}

#[test]
fn parser_plain_lines_stack_traces_are_errors() {
    let out = run_parser(&["fine\n  at x.Y(Z.java)\nCaused by: q\nplain\n\n", "tail"]);
    let levels: Vec<_> = out.iter().map(|(_, l)| *l).collect();
    assert_eq!(
        levels,
        [
            LogLevel::Info,
            LogLevel::Error,
            LogLevel::Error,
            LogLevel::Info,
            LogLevel::Info
        ]
    );
    assert_eq!(out[4].0, "tail");
}

#[test]
fn utf8_chunker_survives_split_characters() {
    let text = "a\u{e4}\u{20ac}\u{1F600}z";
    let bytes = text.as_bytes();
    for cut in 1..bytes.len() {
        let mut c = Utf8Chunker::new();
        let mut s = c.push(&bytes[..cut]);
        s.push_str(&c.push(&bytes[cut..]));
        assert_eq!(s, text, "cut at {cut}");
    }
    let mut c = Utf8Chunker::new();
    assert_eq!(c.push(&[b'a', 0xff, b'b']), "a\u{FFFD}b");
    assert_eq!(c.push(&[0xe2, 0x82]), "");
    assert_eq!(c.finish(), "\u{FFFD}");
}

#[test]
fn log_file_name() {
    let p = log_file_for(
        Path::new("/l"),
        "My  Pack! 1.21",
        "12345678-aaaa-bbbb-cccc-dddddddddddd",
    );
    assert_eq!(p, Path::new("/l/my-pack-1-21-12345678.log"));
    let p = log_file_for(Path::new("/l"), "###", "abcdef012345");
    assert_eq!(p, Path::new("/l/instance-abcdef01.log"));
}

#[test]
fn follower_reads_in_pieces_and_handles_truncation() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("f.log");
    std::fs::write(&file, "").unwrap();
    let got = Arc::new(Mutex::new(String::new()));
    let sink = Arc::clone(&got);
    let f = LogFollower::start(
        &file,
        Duration::from_millis(20),
        FollowStart::Beginning,
        move |t| sink.lock().unwrap().push_str(t),
    );
    let bytes = "h\u{e4}llo\n".as_bytes().to_vec();
    std::fs::write(&file, &bytes[..2]).unwrap(); // cuts the umlaut in half
    std::thread::sleep(Duration::from_millis(80));
    std::fs::write(&file, &bytes).unwrap();
    f.stop();
    assert_eq!(*got.lock().unwrap(), "h\u{e4}llo\n");
}

#[test]
fn follower_start_at_end_skips_existing() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("f.log");
    std::fs::write(&file, "old\n").unwrap();
    let got = Arc::new(Mutex::new(String::new()));
    let sink = Arc::clone(&got);
    let f = LogFollower::start(
        &file,
        Duration::from_millis(20),
        FollowStart::End,
        move |t| sink.lock().unwrap().push_str(t),
    );
    let mut data = std::fs::read(&file).unwrap();
    data.extend_from_slice(b"new\n");
    std::fs::write(&file, data).unwrap();
    f.stop();
    assert_eq!(*got.lock().unwrap(), "new\n");
}

#[cfg(unix)]
mod unix {
    use super::*;
    use mango_minecraft::LaunchCommand;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    fn opts(dir: &Path, prog: PathBuf, cb: EventCb) -> SpawnOptions {
        let cmd = LaunchCommand {
            program: prog,
            args: vec![],
            env: vec![("INST_NAME".into(), "t".into())],
            cwd: dir.to_path_buf(),
        };
        let mut o = SpawnOptions::new(cmd, dir.join("logs/t.log"), "pid", "1.21", cb);
        o.poll_interval = Duration::from_millis(20);
        o.running_fallback = Duration::from_millis(400);
        o
    }

    fn printer() -> String {
        format!(
            "echo '{}'\necho '{}'\necho 'plain'\n",
            event_xml("INFO", "main", "Setting user: Steve"),
            event_xml("WARN", "main", "careful")
        )
    }

    const LOOP: &str = "while :; do sleep 0.05; done";

    #[test]
    fn clean_exit_detects_running_and_parses_log() {
        let dir = tempfile::tempdir().unwrap();
        let java = script(dir.path(), "java", &format!("{}exit 0", printer()));
        let (cb, store) = collect();
        let g = GameInstance::spawn(opts(dir.path(), java, cb)).unwrap();
        let exit = g.wait_exit(Some(Duration::from_secs(10))).unwrap();
        assert_eq!(exit.code, Some(0));
        assert_eq!(exit.state, GameState::Stopped);
        assert!(exit.played_ms.is_some());
        assert_eq!(g.flush_play_time(), None); // already flushed by the exit
        let ev = store.lock().unwrap().clone();
        let run_at = ev.iter().position(|e| *e == GameEvent::Running).unwrap();
        let last = ev.last().unwrap();
        assert!(matches!(last, GameEvent::Exited(_)));
        assert!(run_at < ev.len() - 1);
        let l = lines(&store);
        assert!(l[0].0.ends_with("[main/INFO]: Setting user: Steve"));
        assert!(l[1].0.ends_with("[main/WARN]: careful"));
        assert_eq!(l[2].0, "plain");
        assert_eq!(l[3].0, "Game exited with code 0");
        assert!(dir.path().join("logs/t.log").exists());
    }

    #[test]
    fn crash_exit_code() {
        let dir = tempfile::tempdir().unwrap();
        let java = script(
            dir.path(),
            "java",
            "echo 'Exception in thread main'\nexit 3",
        );
        let (cb, store) = collect();
        let g = GameInstance::spawn(opts(dir.path(), java, cb)).unwrap();
        let exit = g.wait_exit(Some(Duration::from_secs(10))).unwrap();
        assert_eq!(exit.code, Some(3));
        assert_eq!(exit.state, GameState::Crashed);
        let l = lines(&store);
        assert_eq!(l[0], ("Exception in thread main".into(), LogLevel::Error));
        assert_eq!(l[1].1, LogLevel::Error);
        assert!(!store.lock().unwrap().contains(&GameEvent::Running));
    }

    /// Right after fork the child's comm is still the parent's until the
    /// script is exec'd, so the "is it java" check needs a moment.
    fn wait_until_alive(session: &Session) {
        let begun = std::time::Instant::now();
        while !is_session_alive(session) {
            assert!(begun.elapsed() < Duration::from_secs(5), "never alive");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn fallback_marks_running_then_stop() {
        let dir = tempfile::tempdir().unwrap();
        let java = script(dir.path(), "java", LOOP);
        let (cb, store) = collect();
        let g = GameInstance::spawn(opts(dir.path(), java, cb)).unwrap();
        wait_until_alive(&g.session());
        let begun = std::time::Instant::now();
        while !g.is_running() {
            assert!(begun.elapsed() < Duration::from_secs(5), "never running");
            std::thread::sleep(Duration::from_millis(20));
        }
        g.stop().unwrap();
        let exit = g.wait_exit(Some(Duration::from_secs(10))).unwrap();
        assert_eq!(exit.state, GameState::Stopped);
        assert!(g.has_exited());
        assert!(store.lock().unwrap().contains(&GameEvent::Running));
        assert!(!is_session_alive(&g.session()));
    }

    #[test]
    fn stop_refuses_non_java_process() {
        let dir = tempfile::tempdir().unwrap();
        let other = script(dir.path(), "sleeper", LOOP);
        let (cb, _store) = collect();
        let g = GameInstance::spawn(opts(dir.path(), other, cb)).unwrap();
        let err = g.stop().unwrap_err();
        assert_eq!(err.code(), "not_java");
        // Clean up the stray process by hand.
        let _ = std::process::Command::new("kill")
            .args(["-TERM", &g.pid().to_string()])
            .status();
        assert!(g.wait_exit(Some(Duration::from_secs(10))).is_some());
    }

    #[test]
    fn adopts_a_live_session_and_notices_its_death() {
        let dir = tempfile::tempdir().unwrap();
        let java = script(dir.path(), "java", LOOP);
        let (cb, _s) = collect();
        let g = GameInstance::spawn(opts(dir.path(), java, cb)).unwrap();
        let session = g.session();
        wait_until_alive(&session);

        let (cb2, store2) = collect();
        let mut ad = AdoptOptions::new("pid", g.log_file().to_path_buf(), cb2);
        ad.poll_interval = Duration::from_millis(20);
        ad.alive_interval = Duration::from_millis(50);
        let adopted = GameInstance::adopt(&session, ad).unwrap();
        assert!(adopted.is_running());
        assert_eq!(adopted.pid(), session.pid);
        assert_eq!(adopted.session(), session);

        adopted.stop().unwrap();
        let exit = adopted.wait_exit(Some(Duration::from_secs(10))).unwrap();
        assert_eq!(exit.state, GameState::Stopped);
        assert_eq!(exit.code, None);
        assert!(exit.played_ms.is_some());
        assert!(store2
            .lock()
            .unwrap()
            .iter()
            .any(|e| matches!(e, GameEvent::Exited(_))));
        assert!(g.wait_exit(Some(Duration::from_secs(10))).is_some());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn adoption_and_stop_refuse_a_reused_pid_with_another_start_time() {
        let dir = tempfile::tempdir().unwrap();
        let java = script(dir.path(), "java", LOOP);
        let (cb, _s) = collect();
        let g = GameInstance::spawn(opts(dir.path(), java, cb)).unwrap();
        let real = g.session();
        wait_until_alive(&real);
        // Same live JVM pid, but the record says it was launched long ago.
        let stale = Session {
            started_at: real.started_at.saturating_sub(3_600_000),
            ..real.clone()
        };
        assert!(!is_session_alive(&stale));
        let (cb2, _s2) = collect();
        let ad = AdoptOptions::new("pid", g.log_file().to_path_buf(), cb2);
        assert!(GameInstance::adopt(&stale, ad).is_none());
        assert_eq!(
            crate::terminate_java_started(real.pid, Some(stale.started_at))
                .unwrap_err()
                .code(),
            "not_java"
        );
        // The matching record is still adopted, and the process is still alive.
        assert!(is_session_alive(&real));
        g.stop().unwrap();
        assert!(g.wait_exit(Some(Duration::from_secs(10))).is_some());
    }

    #[test]
    fn adoption_rejects_dead_and_foreign_pids() {
        let (cb, _s) = collect();
        let mk = |pid| Session {
            pid,
            version_id: "1.21".into(),
            started_at: 0,
        };
        // This test binary is not java.
        let me = std::process::id();
        let opt = || AdoptOptions::new("p", PathBuf::from("/nonexistent.log"), Arc::clone(&cb));
        assert!(GameInstance::adopt(&mk(me), opt()).is_none());
        assert!(GameInstance::adopt(&mk(0), opt()).is_none());
        assert!(GameInstance::adopt(&mk(4_000_000_000), opt()).is_none());
    }

    #[test]
    fn spawn_failure_is_typed() {
        let dir = tempfile::tempdir().unwrap();
        let (cb, _s) = collect();
        let err = GameInstance::spawn(opts(dir.path(), dir.path().join("nope"), cb))
            .err()
            .unwrap();
        assert_eq!(err.code(), "spawn_failed");
    }
}
