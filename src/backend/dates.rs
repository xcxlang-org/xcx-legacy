use super::runtime;
use crate::sema::dates;

pub fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let shifted_year = if month <= 2 { year - 1 } else { year };
    let era = if shifted_year >= 0 {
        shifted_year
    } else {
        shifted_year - 399
    } / 400;
    let year_of_era = shifted_year - era * 400;
    let month_shift = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * month_shift + 2) / 5 + day - 1;
    let day_of_era = 365 * year_of_era + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

pub fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let day_of_era = z - era * 146097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36524 - day_of_era / 146096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    (if month <= 2 { year + 1 } else { year }, month, day)
}

pub fn today_days() -> i64 {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0);
    seconds.div_euclid(86_400)
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_date_parse(text_ptr: i64, text_len: i64) -> i64 {
    parse(text_ptr, text_len, None)
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_date_parse_format(
    text_ptr: i64,
    text_len: i64,
    format_ptr: i64,
    format_len: i64,
) -> i64 {
    let format = runtime::read_text(format_ptr, format_len);
    parse(text_ptr, text_len, Some(&format))
}

fn parse(text_ptr: i64, text_len: i64, format: Option<&str>) -> i64 {
    let text = runtime::read_text(text_ptr, text_len);
    match dates::parse_date(&text, format) {
        Ok((year, month, day)) => days_from_civil(year, month, day),
        Err(message) => runtime::halt(&message),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_date_now() -> i64 {
    today_days()
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_date_year(days: i64) -> i64 {
    civil_from_days(days).0
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_date_month(days: i64) -> i64 {
    civil_from_days(days).1
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_date_day(days: i64) -> i64 {
    civil_from_days(days).2
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_date_format(
    days: i64,
    format_ptr: i64,
    format_len: i64,
    out_len: *mut i64,
) -> i64 {
    let (year, month, day) = civil_from_days(days);
    let template = runtime::read_text(format_ptr, format_len);
    let rendered = dates::format_date(year, month, day, &template);
    unsafe { *out_len = rendered.len() as i64 };
    super::memory::allocate_bytes(rendered.into_bytes())
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_print_date(days: i64) {
    let (year, month, day) = civil_from_days(days);
    runtime::write_line(&dates::format_date(year, month, day, dates::DEFAULT_FORMAT));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_conversions_round_trip() {
        for days in [-1000, 0, 1, 719_468, 10_000, 100_000, 1_000_000] {
            let (year, month, day) = civil_from_days(days);
            assert_eq!(days_from_civil(year, month, day), days, "{year}-{month}-{day}");
        }
    }

    #[test]
    fn known_dates_map_to_known_days() {
        let christmas = days_from_civil(2024, 12, 25);
        let (year, month, day) = civil_from_days(christmas);
        assert_eq!((year, month, day), (2024, 12, 25));

        let epoch = days_from_civil(1970, 1, 1);
        assert_eq!(epoch, 0);
    }

    #[test]
    fn leap_days_are_contiguous() {
        let before = days_from_civil(2024, 2, 28);
        let leap = days_from_civil(2024, 2, 29);
        let after = days_from_civil(2024, 3, 1);
        assert_eq!(leap - before, 1);
        assert_eq!(after - leap, 1);
    }
}
