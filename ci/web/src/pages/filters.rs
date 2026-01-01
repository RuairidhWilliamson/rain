use askama::{Template, filters::Safe};
use chrono::Utc;

#[askama::filter_fn]
pub fn timestamp(
    timestamp: &chrono::DateTime<Utc>,
    _: &dyn askama::Values,
) -> askama::Result<Safe<String>> {
    #[derive(Template)]
    #[template(
        ext = "html",
        source = "<time class='timestamp' datetime='{{rfc3339}}' title='{{rfc3339}}'>{{timestamp}}</time>"
    )]
    struct TimestampTemplate<'a> {
        timestamp: &'a chrono::DateTime<Utc>,
        rfc3339: String,
    }
    let rfc3339 = timestamp.to_rfc3339();
    Ok(Safe(TimestampTemplate { timestamp, rfc3339 }.render()?))
}

#[askama::filter_fn]
pub fn duration(
    duration: &chrono::TimeDelta,
    _: &dyn askama::Values,
) -> askama::Result<Safe<String>> {
    #[derive(Template)]
    #[template(
        ext = "html",
        source = "<time class='duration' datetime='{{duration}}' title='{{duration}}'>{{duration}}</time>"
    )]
    struct DurationTemplate<'a> {
        duration: &'a chrono::TimeDelta,
    }
    Ok(Safe(DurationTemplate { duration }.render()?))
}
