// Throwaway probe: does each parse path airsl's time.parse uses consult the tz database?
use jiff::fmt::strtime;

fn main() {
    // Path 1: pattern with %Q (airsl: strtime::parse then to_zoned).
    let bdt = strtime::parse("%Y-%m-%d %H:%M %Q", "2024-07-01 12:00 America/New_York").unwrap();
    println!("%Q iana_time_zone() before lookup: {:?}", bdt.iana_time_zone());
    println!("%Q offset() before lookup: {:?}", bdt.offset());
    println!("%Q to_zoned: {:?}", bdt.to_zoned().map(|z| z.timestamp().as_second()));
    // Path 2: pattern with %z only.
    let bdt = strtime::parse("%Y-%m-%d %H:%M %z", "2024-07-01 12:00 -0400").unwrap();
    println!("%z iana_time_zone(): {:?}", bdt.iana_time_zone());
    println!("%z to_zoned: {:?}", bdt.to_zoned().map(|z| z.timestamp().as_second()));
    // Path 3: no pattern, RFC 3339/9557 text with a bracketed zone annotation (airsl: str::parse::<Timestamp>).
    let t: Result<jiff::Timestamp, _> = "2024-07-01T12:00:00-04:00[America/New_York]".parse();
    println!("rfc9557 annotated Timestamp parse: {:?}", t.map(|t| t.as_second()));
    let t: Result<jiff::Timestamp, _> = "2024-07-01T12:00:00-04:00[Not/AZone]".parse();
    println!("rfc9557 unknown-zone Timestamp parse: {:?}", t.map(|t| t.as_second()));
}
