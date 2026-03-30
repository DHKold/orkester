use orkester_plugin::logging::LogRecord;
use super::LogSink;

pub struct ConsoleLogSink;

impl LogSink for ConsoleLogSink {
    fn write(&self, _record: &LogRecord, formatted: &str) -> Result<(), Box<dyn std::error::Error>> {
        println!("{formatted}");
        Ok(())
    }
}
