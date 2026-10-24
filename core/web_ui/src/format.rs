// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use chrono::DateTime;

use crate::models::ResourceOptions;

const MICROS_PER_SECOND: u64 = 1_000_000;

/// Formats a server timestamp (Unix microseconds) in UTC.
pub fn timestamp(micros: u64) -> String {
    format_micros(micros, "%Y-%m-%d %H:%M:%S UTC")
}

/// Like [`timestamp`], keeping the microseconds that order messages.
pub fn timestamp_precise(micros: u64) -> String {
    format_micros(micros, "%Y-%m-%d %H:%M:%S%.6f UTC")
}

/// Renders the `message_expiry` option (microseconds, 0 or `u64::MAX` means
/// never) as a short duration.
pub fn message_expiry(options: &ResourceOptions) -> String {
    let micros = options
        .get("message_expiry")
        .and_then(|option| option.value.parse::<u64>().ok())
        .unwrap_or(0);
    if micros == 0 || micros == u64::MAX {
        return "never".to_owned();
    }
    duration(micros / MICROS_PER_SECOND)
}

/// Renders how long the server has been running, given microseconds.
pub fn elapsed(micros: u64) -> String {
    duration(micros / MICROS_PER_SECOND)
}

/// Re-renders a server byte size such as `"8615888 KiB"` in the largest
/// binary unit that keeps it at or above 1, falling back to the server's text
/// when it does not parse.
pub fn byte_size(server_text: &str) -> String {
    const UNITS: [&str; 6] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
    let Some(mut amount) = parse_byte_size(server_text) else {
        return server_text.to_owned();
    };
    let mut unit = 0;
    while amount >= 1024.0 && unit < UNITS.len() - 1 {
        amount /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{amount} B")
    } else {
        format!("{amount:.2} {}", UNITS[unit])
    }
}

/// Shows the payload as text when it is UTF-8 and as hex otherwise.
pub fn payload(base64_payload: &str) -> String {
    let Ok(bytes) = STANDARD.decode(base64_payload) else {
        return base64_payload.to_owned();
    };
    match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(error) => error
            .as_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<Vec<_>>()
            .join(" "),
    }
}

/// Message IDs are 128-bit and usually UUIDs, so they read best in that form.
pub fn message_id(id: u128) -> String {
    let hex = format!("{id:032x}");
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}

pub fn truncate(text: &str, max_chars: usize) -> String {
    match text.char_indices().nth(max_chars) {
        Some((cut, _)) => format!("{}…", &text[..cut]),
        None => text.to_owned(),
    }
}

fn parse_byte_size(text: &str) -> Option<f64> {
    let (amount, unit) = text.trim().split_once(' ')?;
    let amount: f64 = amount.parse().ok()?;
    let multiplier: f64 = match unit {
        "B" => 1.0,
        "KB" => 1e3,
        "MB" => 1e6,
        "GB" => 1e9,
        "TB" => 1e12,
        "PB" => 1e15,
        "KiB" => 1024.0,
        "MiB" => 1024.0_f64.powi(2),
        "GiB" => 1024.0_f64.powi(3),
        "TiB" => 1024.0_f64.powi(4),
        "PiB" => 1024.0_f64.powi(5),
        _ => return None,
    };
    Some(amount * multiplier)
}

fn format_micros(micros: u64, pattern: &str) -> String {
    i64::try_from(micros)
        .ok()
        .and_then(DateTime::from_timestamp_micros)
        .map(|date| date.format(pattern).to_string())
        .unwrap_or_else(|| micros.to_string())
}

fn duration(total_seconds: u64) -> String {
    const UNITS: [(u64, &str); 4] = [(86_400, "d"), (3_600, "h"), (60, "m"), (1, "s")];
    let mut remaining = total_seconds;
    let parts: Vec<String> = UNITS
        .iter()
        .filter_map(|&(unit_seconds, suffix)| {
            let amount = remaining / unit_seconds;
            remaining %= unit_seconds;
            (amount > 0).then(|| format!("{amount}{suffix}"))
        })
        .collect();
    if parts.is_empty() {
        "0s".to_owned()
    } else {
        parts.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::OptionValue;

    fn expiry_options(micros: u64) -> ResourceOptions {
        ResourceOptions::from([(
            "message_expiry".to_owned(),
            OptionValue {
                value: micros.to_string(),
                kind: "uint64".to_owned(),
                explicit: true,
            },
        )])
    }

    #[test]
    fn given_zero_or_max_expiry_should_render_never() {
        assert_eq!(message_expiry(&expiry_options(0)), "never");
        assert_eq!(message_expiry(&expiry_options(u64::MAX)), "never");
        assert_eq!(message_expiry(&ResourceOptions::new()), "never");
    }

    #[test]
    fn given_expiry_in_micros_should_render_largest_units_first() {
        let micros = (86_400 + 3_600 + 5) * MICROS_PER_SECOND;
        assert_eq!(message_expiry(&expiry_options(micros)), "1d 1h 5s");
    }

    #[test]
    fn given_exact_unit_sizes_should_render_largest_binary_unit() {
        assert_eq!(byte_size("8615888 KiB"), "8.22 GiB");
        assert_eq!(byte_size("16 GiB"), "16.00 GiB");
        assert_eq!(byte_size("3.898 KB"), "3.81 KiB");
        assert_eq!(byte_size("0 B"), "0 B");
        assert_eq!(byte_size("512 B"), "512 B");
    }

    #[test]
    fn given_unparsable_size_should_keep_server_text() {
        assert_eq!(byte_size("lots"), "lots");
        assert_eq!(byte_size("12 parsecs"), "12 parsecs");
    }

    #[test]
    fn given_run_time_in_micros_should_render_duration() {
        assert_eq!(elapsed(4_000_000), "4s");
        assert_eq!(elapsed((3_600 + 120) * MICROS_PER_SECOND), "1h 2m");
    }

    #[test]
    fn given_utf8_payload_should_render_text() {
        assert_eq!(payload(&STANDARD.encode("hello")), "hello");
    }

    #[test]
    fn given_binary_payload_should_render_hex() {
        assert_eq!(payload(&STANDARD.encode([0xff, 0x00, 0x10])), "ff 00 10");
    }

    #[test]
    fn given_message_id_should_render_uuid_form() {
        assert_eq!(
            message_id(0x0123_4567_89ab_cdef_0123_4567_89ab_cdef),
            "01234567-89ab-cdef-0123-456789abcdef"
        );
        assert_eq!(message_id(1), "00000000-0000-0000-0000-000000000001");
    }

    #[test]
    fn given_long_text_should_truncate_on_char_boundary() {
        assert_eq!(truncate("zażółć", 3), "zaż…");
        assert_eq!(truncate("short", 10), "short");
    }

    #[test]
    fn given_micros_should_format_utc() {
        assert_eq!(timestamp(1_700_000_000_123_456), "2023-11-14 22:13:20 UTC");
        assert_eq!(
            timestamp_precise(1_700_000_000_123_456),
            "2023-11-14 22:13:20.123456 UTC"
        );
    }
}
