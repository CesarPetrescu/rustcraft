use std::{
    fs::{create_dir_all, File},
    io::Write,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex, OnceLock,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

struct ProfilerInner {
    file: Mutex<File>,
    frame_counter: AtomicU64,
}

static PROFILER: OnceLock<Arc<ProfilerInner>> = OnceLock::new();

#[derive(Clone)]
pub struct FrameCtx {
    inner: Arc<ProfilerInner>,
    frame_index: u64,
}

pub struct SectionGuard {
    inner: Arc<ProfilerInner>,
    frame_label: String,
    label: &'static str,
    start: Instant,
}

impl Drop for SectionGuard {
    fn drop(&mut self) {
        let duration = self.start.elapsed();
        write_line(&self.inner, &self.frame_label, self.label, duration);
    }
}

pub fn init_session() -> std::io::Result<()> {
    if PROFILER.get().is_some() {
        return Ok(());
    }

    create_dir_all("debug")?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_secs();

    let path = PathBuf::from("debug").join(format!("profile_{timestamp}.csv"));
    let mut file = File::create(path)?;
    writeln!(file, "frame,section,duration_ms")?;

    let inner = Arc::new(ProfilerInner {
        file: Mutex::new(file),
        frame_counter: AtomicU64::new(0),
    });

    let _ = PROFILER.set(inner);
    Ok(())
}

pub fn begin_frame() -> Option<FrameCtx> {
    PROFILER.get().map(|inner| FrameCtx {
        inner: inner.clone(),
        frame_index: inner.frame_counter.fetch_add(1, Ordering::Relaxed),
    })
}

impl FrameCtx {
    pub fn section(&self, label: &'static str) -> SectionGuard {
        SectionGuard {
            inner: self.inner.clone(),
            frame_label: self.frame_index.to_string(),
            label,
            start: Instant::now(),
        }
    }

    pub fn scope<F, R>(&self, label: &'static str, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let guard = self.section(label);
        let result = f();
        drop(guard);
        result
    }
}

pub fn scope<F, R>(ctx: &Option<FrameCtx>, label: &'static str, f: F) -> R
where
    F: FnOnce() -> R,
{
    if let Some(frame) = ctx.as_ref() {
        frame.scope(label, f)
    } else {
        f()
    }
}

pub fn record_background(label: &'static str, duration: Duration) {
    if let Some(inner) = PROFILER.get() {
        write_line(inner, "background", label, duration);
    }
}

fn write_line(inner: &ProfilerInner, frame_label: &str, section: &'static str, duration: Duration) {
    if let Ok(mut file) = inner.file.lock() {
        let _ = writeln!(
            file,
            "{},{},{:.6}",
            frame_label,
            section,
            duration.as_secs_f64() * 1000.0
        );
    }
}
