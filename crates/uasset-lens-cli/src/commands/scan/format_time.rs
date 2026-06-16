pub(super) fn format_utc(unix_secs: u64) -> String {
    let s = unix_secs % 60;
    let m = (unix_secs / 60) % 60;
    let h = (unix_secs / 3600) % 24;
    let mut rem_days = unix_secs / 86400;

    let mut year = 1970u64;
    loop {
        let dy = if is_leap_year(year) { 366u64 } else { 365u64 };
        if rem_days < dy {
            break;
        }
        rem_days -= dy;
        year += 1;
    }

    let month_lengths: [u64; 12] = [
        31,
        if is_leap_year(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1u64;
    for &ml in &month_lengths {
        if rem_days < ml {
            break;
        }
        rem_days -= ml;
        month += 1;
    }
    let day = rem_days + 1;

    format!("{year:04}-{month:02}-{day:02} {h:02}:{m:02}:{s:02} UTC")
}

fn is_leap_year(y: u64) -> bool {
    (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_utc_should_format_epoch_zero_as_unix_origin() {
        assert_eq!(format_utc(0), "1970-01-01 00:00:00 UTC");
    }

    #[test]
    fn format_utc_should_format_one_day_correctly() {
        assert_eq!(format_utc(86400), "1970-01-02 00:00:00 UTC");
    }

    #[test]
    fn format_utc_should_handle_leap_year_correctly() {
        // 1972 is a leap year; 1972-02-29 exists
        // days from 1970-01-01 to 1972-02-29:
        //   1970: 365, 1971: 365, then 31 (Jan) + 29 (Feb day 29) - 1 = 59 days into 1972
        //   total = 365 + 365 + 59 = 789 days → epoch 789 * 86400 = 68169600
        assert_eq!(format_utc(68_169_600), "1972-02-29 00:00:00 UTC");
    }
}
