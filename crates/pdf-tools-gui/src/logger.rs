use chrono::{DateTime, Local};
use log::{Level, LevelFilter, Metadata, Record};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct LogEntry {
    pub timestamp: DateTime<Local>,
    pub level: Level,
    pub target: String,
    pub message: String,
}

#[derive(Clone)]
pub struct AppLogger {
    entries: Arc<Mutex<Vec<LogEntry>>>,
    max_entries: usize,
}

impl AppLogger {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Arc::new(Mutex::new(Vec::new())),
            max_entries,
        }
    }

    pub fn init(self) -> Result<(), log::SetLoggerError> {
        log::set_boxed_logger(Box::new(self.clone()))?;
        log::set_max_level(LevelFilter::Info);
        Ok(())
    }

    pub fn get_entries(&self) -> Vec<LogEntry> {
        self.entries.lock().unwrap().clone()
    }

    pub fn latest_message(&self) -> Option<String> {
        self.entries
            .lock()
            .unwrap()
            .last()
            .map(|entry| entry.message.clone())
    }

    pub fn clear(&self) {
        self.entries.lock().unwrap().clear();
    }
}

impl log::Log for AppLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let entry = LogEntry {
                timestamp: Local::now(),
                level: record.level(),
                target: record.target().to_string(),
                message: format!("{}", record.args()),
            };

            let mut entries = self.entries.lock().unwrap();
            entries.push(entry);

            // Keep only the most recent entries
            if entries.len() > self.max_entries {
                let excess = entries.len() - self.max_entries;
                entries.drain(0..excess);
            }
        }
    }

    fn flush(&self) {}
}
