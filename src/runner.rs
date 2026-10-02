use crossbeam_channel::Sender;
use std::io::Write;
use std::path::Path;
use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex;

/// Events streamed from the command runner to a consumer (CLI terminal or GUI).
#[derive(Debug, Clone)]
pub enum Event {
    /// A line produced on stdout.
    Out(String),
    /// A line produced on stderr.
    Err(String),
    /// The command finished with the given exit status.
    Done(Option<i32>),
    /// Generations loaded asynchronously for the right panel.
    Generations(Vec<crate::generations::Generation>),
}

#[derive(Clone)]
pub struct Sink {
    pub tx: Sender<Event>,
    pub log_file: std::sync::Arc<Mutex<std::fs::File>>,
}

impl Sink {
    /// Record an event: forward to the channel and append to the log file.
    pub async fn record(&self, ev: Event) {
        let _ = self.tx.send(ev.clone());
        self.append_log(&ev).await;
    }

    async fn append_log(&self, ev: &Event) {
        let line = match ev {
            Event::Out(l) => Some(l),
            Event::Err(l) => Some(l),
            Event::Done(_) => None,
            Event::Generations(_) => None,
        };
        if let Some(l) = line {
            let mut f = self.log_file.lock().await;
            let _ = writeln!(f, "{}", l.trim_end());
        }
    }
}

/// Spawn `sh -c <cmdline>` inside `cwd`, streaming stdout + stderr lines
/// concurrently through the given sink. Blocks until the process exits.
pub async fn run(cmdline: &str, cwd: &Path, sink: Sink) -> anyhow::Result<i32> {
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(cmdline)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;

    let out = child.stdout.take();
    let err = child.stderr.take();

    let mut handles = Vec::new();

    if let Some(out) = out {
        let sink = sink.clone();
        handles.push(tokio::spawn(async move {
            let mut lines = BufReader::new(out).lines();
            while let Some(line) = lines.next_line().await.ok().flatten() {
                sink.record(Event::Out(line)).await;
            }
        }));
    }

    if let Some(err) = err {
        let sink = sink.clone();
        handles.push(tokio::spawn(async move {
            let mut lines = BufReader::new(err).lines();
            while let Some(line) = lines.next_line().await.ok().flatten() {
                sink.record(Event::Err(line)).await;
            }
        }));
    }

    let status = child.wait().await?;
    let code = status.code();

    for h in handles {
        let _ = h.await;
    }

    sink.record(Event::Done(code)).await;
    Ok(code.unwrap_or(-1))
}

/// A blocking wrapper used by the headless CLI: spawn a runtime and pump all
/// events to the terminal directly.
pub fn run_cli(cmdline: &str, cwd: &Path, log_file: &Path) -> anyhow::Result<i32> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    runtime.block_on(async move {
        if let Some(dir) = log_file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_file)?;
        let (tx, rx) = crossbeam_channel::unbounded();
        let sink = Sink {
            tx,
            log_file: std::sync::Arc::new(Mutex::new(f)),
        };

        let rx = std::thread::spawn(move || {
            let mut count = 0u32;
            for ev in rx.iter() {
                match ev {
                    Event::Out(l) => println!("{}", l),
                    Event::Err(l) => eprintln!("{}", l),
                    Event::Done(code) => {
                        count = code.map(|c| c as u32).unwrap_or(1);
                        // Signal end of stream.
                        break;
                    }
                    Event::Generations(_) => { /* ignored by CLI */ }
                }
            }
            count
        });

        let code = run(cmdline, cwd, sink).await?;
        let _ = rx.join();
        Ok(code)
    })
}