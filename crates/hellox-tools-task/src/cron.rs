use anyhow::{anyhow, Result};
use chrono::{DateTime, Datelike, Duration, Local, Timelike, Weekday};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CronExpression {
    minute: CronField,
    hour: CronField,
    day_of_month: CronField,
    month: CronField,
    day_of_week: CronField,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CronField {
    any: bool,
    ranges: Vec<CronRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CronRange {
    start: u32,
    end: u32,
    step: u32,
}

pub(crate) fn parse_cron_expression(expression: &str) -> Result<CronExpression> {
    let parts = expression.split_whitespace().collect::<Vec<_>>();
    if parts.len() != 5 {
        return Err(anyhow!(
            "invalid cron expression `{expression}`; expected 5 fields"
        ));
    }

    Ok(CronExpression {
        minute: parse_field(parts[0], 0, 59, false)?,
        hour: parse_field(parts[1], 0, 23, false)?,
        day_of_month: parse_field(parts[2], 1, 31, false)?,
        month: parse_field(parts[3], 1, 12, false)?,
        day_of_week: parse_field(parts[4], 0, 7, true)?,
    })
}

pub(crate) fn next_run_after(
    expression: &CronExpression,
    after: DateTime<Local>,
) -> Option<DateTime<Local>> {
    let mut candidate = after
        .with_second(0)?
        .with_nanosecond(0)?
        .checked_add_signed(Duration::minutes(1))?;
    let limit = candidate.checked_add_signed(Duration::days(366))?;

    while candidate <= limit {
        if expression.matches(candidate) {
            return Some(candidate);
        }
        candidate = candidate.checked_add_signed(Duration::minutes(1))?;
    }
    None
}

pub(crate) fn cron_to_human(expression: &str) -> String {
    format!("{expression} (local time)")
}

impl CronExpression {
    fn matches(&self, time: DateTime<Local>) -> bool {
        if !self.minute.matches(time.minute())
            || !self.hour.matches(time.hour())
            || !self.month.matches(time.month())
        {
            return false;
        }

        let day_of_month = self.day_of_month.matches(time.day());
        let day_of_week = self.day_of_week.matches(day_of_week_value(time.weekday()));

        if self.day_of_month.any || self.day_of_week.any {
            day_of_month && day_of_week
        } else {
            day_of_month || day_of_week
        }
    }
}

impl CronField {
    fn matches(&self, value: u32) -> bool {
        self.any
            || self.ranges.iter().any(|range| {
                value >= range.start
                    && value <= range.end
                    && (value - range.start) % range.step == 0
            })
    }
}

fn parse_field(spec: &str, min: u32, max: u32, allow_sunday_alias: bool) -> Result<CronField> {
    let trimmed = spec.trim();
    if trimmed == "*" {
        return Ok(CronField {
            any: true,
            ranges: Vec::new(),
        });
    }

    let mut ranges = Vec::new();
    for segment in trimmed.split(',') {
        let segment = segment.trim();
        if segment.is_empty() {
            return Err(anyhow!("invalid empty cron segment in `{spec}`"));
        }
        let (base, step) = if let Some((base, step)) = segment.split_once('/') {
            let step = step
                .parse::<u32>()
                .map_err(|_| anyhow!("invalid cron step `{step}`"))?;
            if step == 0 {
                return Err(anyhow!("cron step must be greater than zero"));
            }
            (base, step)
        } else {
            (segment, 1)
        };

        if base == "*" {
            ranges.push(CronRange {
                start: min,
                end: max,
                step,
            });
            continue;
        }

        let (start, end) = if let Some((start, end)) = base.split_once('-') {
            (
                parse_value(start, min, max, allow_sunday_alias)?,
                parse_value(end, min, max, allow_sunday_alias)?,
            )
        } else {
            let value = parse_value(base, min, max, allow_sunday_alias)?;
            (value, value)
        };

        if start > end {
            return Err(anyhow!("invalid cron range `{segment}`"));
        }

        ranges.push(CronRange { start, end, step });
    }

    Ok(CronField { any: false, ranges })
}

fn parse_value(raw: &str, min: u32, max: u32, allow_sunday_alias: bool) -> Result<u32> {
    let value = raw
        .trim()
        .parse::<u32>()
        .map_err(|_| anyhow!("invalid cron value `{raw}`"))?;
    let normalized = if allow_sunday_alias && value == 7 {
        0
    } else {
        value
    };
    if normalized < min || normalized > max {
        return Err(anyhow!("cron value `{raw}` is outside {min}..={max}"));
    }
    Ok(normalized)
}

fn day_of_week_value(weekday: Weekday) -> u32 {
    weekday.num_days_from_sunday()
}

#[cfg(test)]
mod tests {
    use chrono::{Datelike, Local, TimeZone, Timelike};

    use super::{next_run_after, parse_cron_expression};

    #[test]
    fn next_run_finds_every_five_minutes() {
        let expression = parse_cron_expression("*/5 * * * *").expect("parse cron");
        let after = Local
            .with_ymd_and_hms(2026, 4, 12, 10, 1, 0)
            .single()
            .expect("time");
        let next = next_run_after(&expression, after).expect("next run");
        assert_eq!(next.minute(), 5);
        assert_eq!(next.hour(), 10);
    }

    #[test]
    fn next_run_handles_day_of_month_or_week() {
        let expression = parse_cron_expression("0 9 15 * 1").expect("parse cron");
        let after = Local
            .with_ymd_and_hms(2026, 4, 13, 10, 0, 0)
            .single()
            .expect("time");
        let next = next_run_after(&expression, after).expect("next run");
        assert_eq!(next.day(), 15);
        assert_eq!(next.hour(), 9);
    }
}
