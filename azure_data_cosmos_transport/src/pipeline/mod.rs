use time::{
    OffsetDateTime, PrimitiveDateTime,
    format_description::{FormatItem, well_known::Rfc3339},
    macros::format_description,
};

mod authorization_policy;
mod signature_target;

pub use authorization_policy::AuthorizationPolicy;

/// RFC 3339: Date and Time on the Internet: Timestamps.
///
/// <https://www.rfc-editor.org/rfc/rfc3339>
///
/// In [TypeSpec](https://aka.ms/typespec) properties are specified as `utcDateTime` or `offsetDateTime`.
/// In OpenAPI 2.0 specifications properties are specified as `"format": "date-time"`.
///
/// Example string: `1985-04-12T23:20:50.52Z`.
pub fn parse_rfc3339(s: &str) -> anyhow::Result<OffsetDateTime> {
    Ok(OffsetDateTime::parse(s, &Rfc3339)?)
}

/// RFC 3339: Date and Time on the Internet: Timestamps.
///
/// <https://www.rfc-editor.org/rfc/rfc3339>
///
/// In [TypeSpec](https://aka.ms/typespec) properties are specified as `utcDateTime` or `offsetDateTime`.
/// In OpenAPI 2.0 specifications properties are specified as `"format": "date-time"`.
///
/// Example string: `1985-04-12T23:20:50.52Z`.
pub fn to_rfc3339(date: &OffsetDateTime) -> String {
    // known format does not panic
    date.format(&Rfc3339).unwrap()
}

/// RFC 7231: Requirements for Internet Hosts - Application and Support.
///
/// <https://datatracker.ietf.org/doc/html/rfc7231#section-7.1.1.1>
///
/// In [TypeSpec](https://aka.ms/typespec) headers are specified as `utcDateTime`.
/// In REST API specifications headers are specified as `"format": "date-time-rfc1123"`.
///
/// This format is also the preferred HTTP date-based header format.
/// * <https://datatracker.ietf.org/doc/html/rfc7231#section-7.1.1.2>
/// * <https://datatracker.ietf.org/doc/html/rfc7232>
///
/// Example string: `Sun, 06 Nov 1994 08:49:37 GMT`.
pub fn parse_rfc7231(s: &str) -> anyhow::Result<OffsetDateTime> {
    Ok(PrimitiveDateTime::parse(s, RFC7231_FORMAT)?.assume_utc())
}

const RFC7231_FORMAT: &[FormatItem] = format_description!(
    "[weekday repr:short], [day] [month repr:short] [year] [hour]:[minute]:[second] GMT"
);

/// RFC 7231: Requirements for Internet Hosts - Application and Support.
///
/// <https://datatracker.ietf.org/doc/html/rfc7231#section-7.1.1.1>
///
/// In [TypeSpec](https://aka.ms/typespec) headers are specified as `utcDateTime`.
/// In REST API specifications headers are specified as `"format": "date-time-rfc1123"`.
///
/// This format is also the preferred HTTP date-based header format.
/// * <https://datatracker.ietf.org/doc/html/rfc7231#section-7.1.1.2>
/// * <https://datatracker.ietf.org/doc/html/rfc7232>
///
/// Example string: `Sun, 06 Nov 1994 08:49:37 GMT`.
pub fn to_rfc7231(date: &OffsetDateTime) -> String {
    // known format does not panic
    date.format(&RFC7231_FORMAT).unwrap()
}
