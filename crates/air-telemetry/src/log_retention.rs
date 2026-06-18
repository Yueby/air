use std::fs::{self, OpenOptions};
use std::io;
use std::path::Path;

use time::format_description::well_known::Rfc3339;
use time::{Date, Duration as TimeDuration, Month, OffsetDateTime, UtcOffset};

pub const LOG_RETENTION_DAYS: i64 = 3;

pub fn prepare_managed_log_for_append(path: &Path) -> io::Result<()> {
    prepare_managed_log_for_append_at(path, local_now())
}

pub fn format_current_log_timestamp() -> String {
    format_log_timestamp(local_now())
}

pub fn format_log_timestamp(now: OffsetDateTime) -> String {
    now.format(&Rfc3339)
        .unwrap_or_else(|_| now.unix_timestamp().to_string())
}

pub fn local_now() -> OffsetDateTime {
    OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc())
}

pub(crate) fn archive_name(stem: &str, date: Date) -> String {
    format!(
        "{stem}-{:04}-{:02}-{:02}.log",
        date.year(),
        u8::from(date.month()),
        date.day()
    )
}

pub(crate) fn prepare_managed_log_for_append_at(
    path: &Path,
    now: OffsetDateTime,
) -> io::Result<()> {
    let today = now.date();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
        rotate_stale_current_log(path, today, now.offset())?;
        prune_expired_archives(parent, log_stem(path), today)?;
    }
    Ok(())
}

fn rotate_stale_current_log(path: &Path, today: Date, offset: UtcOffset) -> io::Result<()> {
    if !path.is_file() {
        return Ok(());
    }
    let Some(modified_date) = file_modified_date(path, offset)? else {
        return Ok(());
    };
    if modified_date >= today {
        return Ok(());
    }
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    let archive = parent.join(archive_name(log_stem(path), modified_date));
    append_file_to_archive(path, &archive)?;
    fs::remove_file(path)?;
    Ok(())
}

pub(crate) fn prune_expired_archives(
    logs_dir: &Path,
    stem: &str,
    today: Date,
) -> io::Result<usize> {
    let cutoff = today - TimeDuration::days(LOG_RETENTION_DAYS - 1);
    let mut removed = 0;
    if !logs_dir.is_dir() {
        return Ok(removed);
    }
    for entry in fs::read_dir(logs_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(date) = archive_date(stem, &path) else {
            continue;
        };
        if date < cutoff {
            fs::remove_file(path)?;
            removed += 1;
        }
    }
    Ok(removed)
}

fn append_file_to_archive(source: &Path, archive: &Path) -> io::Result<()> {
    let mut source_file = match fs::File::open(source) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    let mut archive_file = OpenOptions::new().create(true).append(true).open(archive)?;
    io::copy(&mut source_file, &mut archive_file)?;
    Ok(())
}

fn file_modified_date(path: &Path, offset: UtcOffset) -> io::Result<Option<Date>> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let modified = metadata.modified()?;
    let modified = OffsetDateTime::from(modified).to_offset(offset);
    Ok(Some(modified.date()))
}

fn log_stem(path: &Path) -> &str {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("log")
}

fn archive_date(stem: &str, path: &Path) -> Option<Date> {
    let file_name = path.file_name()?.to_str()?;
    let prefix = format!("{stem}-");
    let suffix = ".log";
    let date = file_name.strip_prefix(&prefix)?.strip_suffix(suffix)?;
    let mut parts = date.split('-');
    let year = parts.next()?.parse::<i32>().ok()?;
    let month = parts.next()?.parse::<u8>().ok()?;
    let day = parts.next()?.parse::<u8>().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Date::from_calendar_date(year, Month::try_from(month).ok()?, day).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(year: i32, month: u8, day: u8) -> Date {
        Date::from_calendar_date(year, Month::try_from(month).unwrap(), day).unwrap()
    }

    #[test]
    fn archive_name_uses_stable_calendar_date() {
        assert_eq!(
            archive_name("core", date(2026, 6, 2)),
            "core-2026-06-02.log"
        );
    }

    #[test]
    fn prune_keeps_current_and_previous_two_days() {
        let temp = tempfile::tempdir().unwrap();
        for name in [
            "core-2026-05-30.log",
            "core-2026-05-31.log",
            "core-2026-06-01.log",
            "core-2026-06-02.log",
            "air-2026-05-30.log",
        ] {
            fs::write(temp.path().join(name), name).unwrap();
        }

        let removed = prune_expired_archives(temp.path(), "core", date(2026, 6, 2)).unwrap();

        assert_eq!(removed, 1);
        assert!(!temp.path().join("core-2026-05-30.log").exists());
        assert!(temp.path().join("core-2026-05-31.log").exists());
        assert!(temp.path().join("core-2026-06-01.log").exists());
        assert!(temp.path().join("core-2026-06-02.log").exists());
        assert!(temp.path().join("air-2026-05-30.log").exists());
    }
}
