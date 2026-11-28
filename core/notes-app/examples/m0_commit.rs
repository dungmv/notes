use notes_app::editor::{Batch, Editor, FIXTURE, Operation, Snapshot};
use rusqlite::{Connection, params};
use std::error::Error;
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const OPS: &str = include_str!("../../../fixtures/m0/operations-v1.json");
const EXPECTED: &str = include_str!("../../../fixtures/m0/expected-v1.json");
const INTERVAL: Duration = Duration::from_millis(4);
type Result<T> = std::result::Result<T, Box<dyn Error>>;

struct Edit {
    arrival: Instant,
    revision: i64,
    operation: String,
    snapshot: String,
}

fn open(path: &Path) -> Result<Connection> {
    let db = Connection::open(path)?;
    db.execute_batch(
        "PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
         CREATE TABLE IF NOT EXISTS journal(revision INTEGER PRIMARY KEY, operation TEXT NOT NULL);
         CREATE TABLE IF NOT EXISTS document(id INTEGER PRIMARY KEY CHECK(id=1), snapshot TEXT NOT NULL);",
    )?;
    let mode: String = db.query_row("PRAGMA journal_mode", [], |row| row.get(0))?;
    let synchronous: u32 = db.query_row("PRAGMA synchronous", [], |row| row.get(0))?;
    assert_eq!((mode.as_str(), synchronous), ("wal", 2));
    Ok(db)
}

fn edit(editor: &mut Editor, arrival: Instant, operation: Operation) -> Result<Edit> {
    let encoded = serde_json::to_string(&operation)?;
    let updated = editor.apply(Batch {
        revision: editor.snapshot().revision,
        operations: vec![operation],
        external: false,
    })?;
    Ok(Edit {
        arrival,
        revision: i64::try_from(updated.revision)?,
        operation: encoded,
        snapshot: serde_json::to_string(editor.snapshot())?,
    })
}

fn flush(db: &mut Connection, pending: &mut Vec<Edit>) -> Result<Vec<f64>> {
    let Some(last) = pending.last() else {
        return Ok(Vec::new());
    };
    let transaction = db.transaction()?;
    for edit in pending.iter() {
        transaction.execute(
            "INSERT INTO journal(revision, operation) VALUES (?1, ?2)",
            params![edit.revision, edit.operation],
        )?;
    }
    transaction.execute(
        "INSERT INTO document VALUES (1, ?1) ON CONFLICT(id) DO UPDATE SET snapshot=excluded.snapshot",
        [&last.snapshot],
    )?;
    transaction.commit()?;
    let ack = Instant::now();
    Ok(pending
        .drain(..)
        .map(|edit| ack.duration_since(edit.arrival).as_secs_f64() * 1000.0)
        .collect())
}

fn verify(db: &Connection, expected: &Snapshot) -> Result<()> {
    let snapshot: String = db.query_row("SELECT snapshot FROM document", [], |row| row.get(0))?;
    assert_eq!(&serde_json::from_str::<Snapshot>(&snapshot)?, expected);
    let mut query = db.prepare("SELECT revision, operation FROM journal ORDER BY revision")?;
    let entries = query
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let operations: Vec<Operation> = serde_json::from_str(OPS)?;
    assert_eq!(entries.len(), expected.revision as usize);
    for (index, ((revision, actual), operation)) in entries.iter().zip(operations).enumerate() {
        assert_eq!(*revision, i64::try_from(index)? + 1);
        assert_eq!(*actual, serde_json::to_string(&operation)?);
    }
    Ok(())
}

fn wait_until(deadline: Instant) {
    thread::sleep(deadline.saturating_duration_since(Instant::now()));
}

fn run(db: &mut Connection, window: Duration) -> Result<(Vec<f64>, Vec<usize>)> {
    let mut editor = Editor::open(FIXTURE)?;
    let operations: Vec<Operation> = serde_json::from_str(OPS)?;
    let mut pending: Vec<Edit> = Vec::new();
    let mut latencies = Vec::new();
    let mut sizes = Vec::new();
    let start = Instant::now();
    for (index, operation) in operations.into_iter().enumerate() {
        let arrival = start + INTERVAL * index as u32;
        if let Some(first) = pending.first() {
            let deadline = first.arrival + window;
            if deadline <= arrival || deadline <= Instant::now() {
                wait_until(deadline);
                sizes.push(pending.len());
                latencies.extend(flush(db, &mut pending)?);
            }
        }
        wait_until(arrival);
        pending.push(edit(&mut editor, arrival, operation)?);
        if window.is_zero() {
            sizes.push(pending.len());
            latencies.extend(flush(db, &mut pending)?);
        }
    }
    if !pending.is_empty() {
        sizes.push(pending.len());
        latencies.extend(flush(db, &mut pending)?);
    }
    Ok((latencies, sizes))
}

fn verify_failure_and_retry(path: &Path, expected: &Snapshot) -> Result<()> {
    let mut db = open(path)?;
    let mut editor = Editor::open(FIXTURE)?;
    let operations: Vec<Operation> = serde_json::from_str(OPS)?;
    let mut edits = operations
        .into_iter()
        .map(|operation| edit(&mut editor, Instant::now(), operation))
        .collect::<Result<Vec<_>>>()?;
    let prefix: Snapshot = serde_json::from_str(&edits[0].snapshot)?;
    let mut first = vec![edits.remove(0)];
    assert_eq!(flush(&mut db, &mut first)?.len(), 1);
    db.execute_batch(
        "CREATE TEMP TRIGGER reject_write BEFORE INSERT ON journal WHEN NEW.revision = 3
         BEGIN SELECT RAISE(ABORT, 'injected write failure'); END;",
    )?;
    assert!(flush(&mut db, &mut edits).is_err());
    assert_eq!(edits.len(), expected.revision as usize - 1);
    verify(&open(path)?, &prefix)?;
    db.execute_batch("DROP TRIGGER reject_write")?;
    assert_eq!(
        flush(&mut db, &mut edits)?.len(),
        expected.revision as usize - 1
    );
    assert!(edits.is_empty());
    verify(&open(path)?, expected)
}

fn verify_process_stop(path: &Path, window_ms: u64, expected: &Snapshot) -> Result<()> {
    let mut child = Command::new(std::env::current_exe()?)
        .arg("--write-child")
        .arg(path)
        .arg(window_ms.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    let mut line = String::new();
    let received =
        io::BufReader::new(child.stdout.take().expect("piped stdout")).read_line(&mut line);
    let stopped = child.kill();
    child.wait()?;
    received?;
    stopped?;
    assert_eq!(line.trim(), "committed");
    verify(&open(path)?, expected)
}

fn percentile(sorted: &[f64], percent: usize) -> f64 {
    sorted[(sorted.len() * percent).div_ceil(100) - 1]
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|arg| arg == "--write-child") {
        let mut db = open(Path::new(&args[2]))?;
        let window = Duration::from_millis(args[3].to_string_lossy().parse()?);
        run(&mut db, window)?;
        println!("committed");
        io::stdout().flush()?;
        io::stdin().lock().read_line(&mut String::new())?;
        return Ok(());
    }

    let root = Path::new("target").join("m0-commit");
    std::fs::create_dir_all(&root)?;
    let directory = root.join(format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis()
    ));
    std::fs::create_dir(&directory)?;
    let expected: Snapshot = serde_json::from_str(EXPECTED)?;
    println!(
        "SQLite {}; files: {}",
        rusqlite::version(),
        directory.display()
    );
    for window_ms in [0, 12] {
        let mut all = Vec::new();
        let mut sizes = std::collections::BTreeMap::new();
        for round in 0..30 {
            let mut db = open(&directory.join(format!("{window_ms}-{round}.db")))?;
            let (latencies, commits) = run(&mut db, Duration::from_millis(window_ms))?;
            verify(&db, &expected)?;
            all.extend(latencies);
            for size in commits {
                *sizes.entry(size).or_insert(0) += 1;
            }
        }
        all.sort_by(f64::total_cmp);
        println!(
            "{window_ms} ms: 30 runs, {} edits, batch histogram {sizes:?}, p50 {:.3} ms, p95 {:.3} ms",
            all.len(),
            percentile(&all, 50),
            percentile(&all, 95)
        );
        verify_process_stop(
            &directory.join(format!("recovery-{window_ms}.db")),
            window_ms,
            &expected,
        )?;
    }
    verify_failure_and_retry(&directory.join("retry.db"), &expected)?;
    println!("PASS: post-commit ack, final flush, replay order, process stop, rollback and retry");
    Ok(())
}
