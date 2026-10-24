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

//! Preferences for this browser: the colour theme and how often pages
//! reload their data.

use std::str::FromStr;

use gloo_storage::{LocalStorage, Storage};
use serde::{Deserialize, Serialize};

const SETTINGS_KEY: &str = "iggy_ui_settings";

pub const MIN_REFRESH_INTERVAL_MS: u32 = 500;
pub const MAX_REFRESH_INTERVAL_MS: u32 = 3_600_000;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    /// Follows the operating system's light or dark preference.
    #[default]
    System,
    Light,
    Dark,
}

impl Theme {
    pub const ALL: [Self; 3] = [Self::System, Self::Light, Self::Dark];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }
}

impl FromStr for Theme {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|theme| theme.as_str() == text)
            .ok_or_else(|| format!("unknown theme {text}"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiSettings {
    pub theme: Theme,
    pub refresh_interval_ms: u32,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            refresh_interval_ms: 4_000,
        }
    }
}

impl UiSettings {
    /// Falls back to the defaults when nothing is stored or the stored
    /// interval is out of range.
    pub fn restore() -> Self {
        LocalStorage::get::<Self>(SETTINGS_KEY)
            .ok()
            .filter(|settings| {
                parse_refresh_interval(&settings.refresh_interval_ms.to_string()).is_ok()
            })
            .unwrap_or_default()
    }

    pub fn persist(&self) {
        // A failed write only means the defaults come back after a reload.
        let _ = LocalStorage::set(SETTINGS_KEY, self);
    }
}

pub fn parse_refresh_interval(text: &str) -> Result<u32, String> {
    match text.trim().parse::<u32>() {
        Ok(ms) if (MIN_REFRESH_INTERVAL_MS..=MAX_REFRESH_INTERVAL_MS).contains(&ms) => Ok(ms),
        _ => Err(format!(
            "Enter a whole number of milliseconds from {MIN_REFRESH_INTERVAL_MS} to {MAX_REFRESH_INTERVAL_MS}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_interval_in_range_should_accept_it() {
        assert_eq!(parse_refresh_interval(" 500 "), Ok(500));
        assert_eq!(parse_refresh_interval("3600000"), Ok(3_600_000));
    }

    #[test]
    fn given_interval_out_of_range_or_not_a_number_should_refuse_it() {
        for text in ["499", "3600001", "-1", "1.5", "soon", ""] {
            assert!(parse_refresh_interval(text).is_err(), "{text}");
        }
    }

    #[test]
    fn given_theme_name_should_round_trip() {
        for theme in Theme::ALL {
            assert_eq!(theme.as_str().parse::<Theme>(), Ok(theme));
        }
        assert!("sepia".parse::<Theme>().is_err());
    }
}
