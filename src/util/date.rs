use chrono::{DateTime, FixedOffset, Local, NaiveDateTime};

const COMMON_DATE_FORMATS: &[&str] = &[
  "%Y-%m-%d %H:%M:%S",    // Common format without timezone
  "%Y-%m-%d %H:%M:%S %z", // Common format with timezone
  "%Y-%m-%d",             // Date only
];

pub fn parse_date(date_str: impl AsRef<str>) -> Option<DateTime<FixedOffset>> {
  let date_str = date_str.as_ref();
  if date_str.trim().is_empty() {
    return None;
  }

  if let Ok(parsed) = DateTime::parse_from_rfc3339(date_str) {
    return Some(parsed);
  }

  if let Ok(parsed) = DateTime::parse_from_rfc2822(date_str) {
    return Some(parsed);
  }

  for fmt in COMMON_DATE_FORMATS {
    if let Ok(parsed) = DateTime::parse_from_str(date_str, fmt) {
      return Some(parsed);
    }

    if let Ok(parsed) = NaiveDateTime::parse_from_str(date_str, fmt) {
      // try local time, fallback to UTC
      let date = parsed
        .and_local_timezone(Local)
        .earliest()
        .map(|date| date.fixed_offset())
        .unwrap_or_else(|| parsed.and_utc().fixed_offset());
      return Some(date);
    }
  }

  None
}
