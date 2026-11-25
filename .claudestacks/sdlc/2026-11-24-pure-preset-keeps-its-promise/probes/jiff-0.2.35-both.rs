// Throwaway probe: a pattern with both %z and %Q still names the zone, and to_zoned reads it.
use jiff::fmt::strtime;
fn main() {
    let bdt = strtime::parse("%Y-%m-%d %H:%M %z %Q", "2024-07-01 12:00 -0400 America/New_York").unwrap();
    println!("both iana_time_zone(): {:?}", bdt.iana_time_zone());
    println!("both to_zoned: {:?}", bdt.to_zoned().map(|z| z.timestamp().as_second()));
}
