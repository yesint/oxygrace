//! Julian-date to calendar conversions for date/time tick-label formats,
//! ported from Grace's `dates.cpp` / `utils.cpp`.
//!
//! Grace stores date axes as astronomical Julian dates (JD 2447892.5 =
//! 1990-01-01 00:00). The calendar conversion uses the standard
//! Fliegel–Van Flandern algorithm, which is exact for the Gregorian range
//! (JD >= 2299161, i.e. dates after 1582); Grace's extra branches for
//! proleptic Julian dates before that are not replicated.

/// Rounding targets for [`jul_to_cal_and_time`] (Grace `ROUND_*`).
pub const ROUND_SECOND: i32 = 1;
pub const ROUND_DAY: i32 = 4;
pub const ROUND_MONTH: i32 = 5;

pub const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
pub const MONTHL: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
pub const DAYOFWEEKS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
pub const DAYOFWEEKL: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

/// Calendar day number -> (year, month 1..12, day) for the Gregorian range
/// (Fliegel–Van Flandern; `n` is the Julian day number at noon).
fn jul_to_cal(n: i64) -> (i32, u32, u32) {
    let l = n + 68569;
    let nn = 4 * l / 146097;
    let l = l - (146097 * nn + 3) / 4;
    let i = 4000 * (l + 1) / 1461001;
    let l = l - 1461 * i / 4 + 31;
    let j = 80 * l / 2447;
    let d = l - 2447 * j / 80;
    let l = j / 11;
    let m = j + 2 - 12 * l;
    let y = 100 * (nn - 49) + i + l;
    (y as i32, m as u32, d as u32)
}

/// (year, month, day) -> Julian day number at noon.
pub fn cal_to_jul(y: i32, m: u32, d: u32) -> i64 {
    let (y, m, d) = (y as i64, m as i64, d as i64);
    d - 32075 + 1461 * (y + 4800 + (m - 14) / 12) / 4 + 367 * (m - 2 - (m - 14) / 12 * 12) / 12
        - 3 * ((y + 4900 + (m - 14) / 12) / 100) / 4
}

/// Julian date -> calendar and time-of-day elements with Grace's rounding
/// cascade (`jul_to_cal_and_time_with_yday`, dates.cpp): seconds round into
/// minutes, minutes into hours, hours into days as requested by `rounding`.
pub fn jul_to_cal_and_time(jday: f64, rounding: i32) -> (i32, u32, u32, u32, u32, u32) {
    let mut n = (jday + 0.5).floor() as i64;
    let mut tmp = 24.0 * (jday + 0.5 - n as f64);
    let mut hour = tmp.floor() as i64;
    tmp = 60.0 * (tmp - hour as f64);
    let mut min = tmp.floor() as i64;
    tmp = 60.0 * (tmp - min as f64);
    let mut sec = (tmp + 0.5).floor() as i64;

    if sec >= 60 || rounding > ROUND_SECOND {
        if sec >= 30 {
            min += 1;
        }
        sec = 0;
        if min == 60 || rounding > 2 {
            if min >= 30 {
                hour += 1;
            }
            min = 0;
            if hour == 24 || rounding > 3 {
                if hour >= 12 {
                    n += 1;
                }
                hour = 0;
            }
        }
    }

    let (mut y, mut m, mut d) = jul_to_cal(n);
    if rounding == ROUND_MONTH {
        // Round to the nearer month boundary.
        let (m2, y2) = if m < 12 { (m + 1, y) } else { (1, y + 1) };
        let this_first = cal_to_jul(y, m, 1);
        let next_first = cal_to_jul(y2, m2, 1);
        if n - this_first >= next_first - n {
            y = y2;
            m = m2;
        }
        d = 1;
    }
    (y, m, d, hour as u32, min as u32, sec as u32)
}

/// Day of week for a Julian date: 0 = Sunday (utils.cpp `dayofweek`).
pub fn dayofweek(j: f64) -> usize {
    let i = (j + 1.5).floor() as i64;
    (if i <= 0 { 6 - (6 - i) % 7 } else { i % 7 }) as usize % 7
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn julian_dates() {
        // JD 2447892.5 = 1990-01-01 00:00.
        assert_eq!(jul_to_cal_and_time(2447892.5, ROUND_DAY), (1990, 1, 1, 0, 0, 0));
        // times.agr's first panel starts near JD 2447905.5 = 1990-01-14.
        assert_eq!(jul_to_cal_and_time(2447905.5, ROUND_DAY), (1990, 1, 14, 0, 0, 0));
        // Round-trip.
        assert_eq!(cal_to_jul(1990, 1, 14), 2447906); // noon JD number
        // 2448304.5 = 1991-02-17 00:00; 12:00:00 same day.
        assert_eq!(jul_to_cal_and_time(2448304.0, ROUND_SECOND), (1991, 2, 16, 12, 0, 0));
        // 1990-01-14 was a Sunday.
        assert_eq!(dayofweek(2447905.5), 0);
    }
}
