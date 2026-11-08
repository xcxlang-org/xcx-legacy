pub const DEFAULT_FORMAT: &str = "YYYY-MM-DD";

pub fn validate_date_literal(text: &str, format: Option<&str>) -> Result<(), String> {
    let template = format.unwrap_or(DEFAULT_FORMAT);
    let (year, month, day) = extract_components(text, template)?;
    validate_components(year, month, day)
}

pub fn parse_date(text: &str, format: Option<&str>) -> Result<(i64, i64, i64), String> {
    let template = format.unwrap_or(DEFAULT_FORMAT);
    let (year, month, day) = extract_components(text, template)?;
    validate_components(year, month, day)?;
    Ok((year, month, day))
}

pub fn format_date(year: i64, month: i64, day: i64, template: &str) -> String {
    let mut rendered = String::new();
    let scan = scan_template(template, &mut |token, _, character| {
        let piece = match token {
            Token::Year => format!("{year:04}"),
            Token::Month => format!("{month:02}"),
            Token::Day => format!("{day:02}"),
            Token::MonthShort => month.to_string(),
            Token::DayShort => day.to_string(),
            Token::Separator => character.to_string(),
        };
        rendered.push_str(&piece);
    });
    match scan {
        Ok(()) => rendered,
        Err(_) => {
            let _ = &rendered;
            String::new()
        }
    }
}

pub fn validate_format_template(template: &str) -> Result<(), String> {
    scan_template(template, &mut |_, _, _| {})
}

fn scan_template(
    template: &str,
    emit: &mut dyn FnMut(Token, usize, char),
) -> Result<(), String> {
    let characters: Vec<char> = template.chars().collect();
    let mut position = 0;
    while position < characters.len() {
        let character = characters[position];
        let token = match character {
            'Y' if characters[position..].starts_with(&['Y', 'Y', 'Y', 'Y']) => Token::Year,
            'M' if characters.get(position + 1) == Some(&'M') => Token::Month,
            'D' if characters.get(position + 1) == Some(&'D') => Token::Day,
            'M' => Token::MonthShort,
            'D' => Token::DayShort,
            'H' | 'S' | 'T' => {
                return Err(format!("unknown format token `{character}` in `{template}`"));
            }
            other if other.is_ascii_alphabetic() => {
                return Err(format!("unknown format token `{other}` in `{template}`"));
            }
            _ => Token::Separator,
        };
        let width = match token {
            Token::Year => 4,
            Token::Month | Token::Day => 2,
            _ => 1,
        };
        emit(token, width, character);
        position += width;
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq)]
enum Token {
    Year,
    Month,
    Day,
    MonthShort,
    DayShort,
    Separator,
}

fn extract_components(text: &str, template: &str) -> Result<(i64, i64, i64), String> {
    let mut year = None;
    let mut month = None;
    let mut day = None;
    let mut text_position = 0usize;
    let mut error: Option<String> = None;
    let text_characters: Vec<char> = text.chars().collect();

    scan_template(template, &mut |token, _, character| {
        if error.is_some() {
            return;
        }
        if token == Token::Separator {
            match text_characters.get(text_position) {
                Some(&current) if current == character => text_position += 1,
                _ => {
                    error = Some(format!("expected `{character}` at position {text_position}"));
                }
            }
            return;
        }
        let digits: String = text_characters[text_position..]
            .iter()
            .take_while(|current| current.is_ascii_digit())
            .take(digit_width(token))
            .collect();
        if digits.is_empty() {
            error = Some(format!("a numeric component is missing at position {text_position}"));
            return;
        }
        text_position += digits.chars().count();
        let value = digits.parse::<i64>().unwrap_or(-1);
        match token {
            Token::Year => year = Some(value),
            Token::Month | Token::MonthShort => month = Some(value),
            Token::Day | Token::DayShort => day = Some(value),
            Token::Separator => {}
        }
    })
    .map_err(|scan_error| format!("invalid date `{text}`: {scan_error}"))?;

    if let Some(message) = error {
        return Err(format!(
            "invalid date `{text}` for the format `{template}`: {message}"
        ));
    }
    if text_position != text_characters.len() {
        return Err(format!(
            "invalid date `{text}` for the format `{template}`: unexpected text after the components"
        ));
    }

    match (year, month, day) {
        (Some(year), Some(month), Some(day)) => Ok((year, month, day)),
        _ => Err(format!(
            "invalid date `{text}` for the format `{template}`: components are missing"
        )),
    }
}

fn digit_width(token: Token) -> usize {
    match token {
        Token::Year => 4,
        Token::Month | Token::Day | Token::MonthShort | Token::DayShort => 2,
        Token::Separator => 0,
    }
}

fn validate_components(year: i64, month: i64, day: i64) -> Result<(), String> {
    if !(1..=9999).contains(&year) {
        return Err(format!("invalid date: year {year} is outside 1-9999"));
    }
    if !(1..=12).contains(&month) {
        return Err(format!("invalid date: month {month} is outside 1-12"));
    }
    let last_day = days_in_month(year, month);
    if !(1..=last_day).contains(&day) {
        return Err(format!(
            "invalid date: day {day} is outside 1-{last_day} for {year}-{month:02}"
        ));
    }
    Ok(())
}

fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn is_leap_year(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_dates_are_validated() {
        assert!(validate_date_literal("2024-12-25", None).is_ok());
        assert!(validate_date_literal("2024-02-29", None).is_ok());
    }

    #[test]
    fn leap_years_are_respected() {
        assert!(validate_date_literal("2023-02-29", None).is_err());
        assert!(validate_date_literal("2000-02-29", None).is_ok());
        assert!(validate_date_literal("1900-02-29", None).is_err());
    }

    #[test]
    fn component_ranges_are_enforced() {
        assert!(validate_date_literal("2024-13-45", None).is_err());
        assert!(validate_date_literal("0000-01-01", None).is_err());
        assert!(validate_date_literal("2024-04-31", None).is_err());
    }

    #[test]
    fn custom_formats_extract_the_components() {
        assert!(validate_date_literal("25/12/2024", Some("DD/MM/YYYY")).is_ok());
        assert!(validate_date_literal("12/25/2024", Some("MM/DD/YYYY")).is_ok());
        assert!(validate_date_literal("2024/12/25", Some("DD/MM/YYYY")).is_err());
    }

    #[test]
    fn unknown_format_tokens_are_rejected() {
        assert!(validate_format_template("HH-MM-YYYY").is_err());
        assert!(validate_format_template("DD-MM-YYYY").is_ok());
    }

    #[test]
    fn dates_render_through_format_tokens() {
        assert_eq!(format_date(2024, 12, 25, "YYYY-MM-DD"), "2024-12-25");
        assert_eq!(format_date(2024, 12, 25, "DD-MM-YYYY"), "25-12-2024");
        assert_eq!(format_date(2024, 3, 7, "M/D/YYYY"), "3/7/2024");
    }
}
