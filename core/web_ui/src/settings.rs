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

use serde::{Deserialize, Serialize};

use crate::storage;

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
    /// Falls back to the defaults when nothing is stored, storage is blocked
    /// or the stored interval is out of range.
    pub fn restore() -> Self {
        storage::read::<Self>(SETTINGS_KEY)
            .filter(|settings| RefreshInterval::try_from(settings.refresh_interval_ms).is_ok())
            .unwrap_or_default()
    }

    pub fn persist(&self) {
        // A failed write only means the defaults come back after a reload.
        storage::write(SETTINGS_KEY, self);
    }
}

/// How often pages reload their data, in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefreshInterval(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error(
    "Enter a whole number of milliseconds from {MIN_REFRESH_INTERVAL_MS} to {MAX_REFRESH_INTERVAL_MS}"
)]
pub struct InvalidRefreshInterval;

impl TryFrom<u32> for RefreshInterval {
    type Error = InvalidRefreshInterval;

    fn try_from(ms: u32) -> Result<Self, Self::Error> {
        if (MIN_REFRESH_INTERVAL_MS..=MAX_REFRESH_INTERVAL_MS).contains(&ms) {
            Ok(Self(ms))
        } else {
            Err(InvalidRefreshInterval)
        }
    }
}

impl FromStr for RefreshInterval {
    type Err = InvalidRefreshInterval;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let ms = text
            .trim()
            .parse::<u32>()
            .map_err(|_| InvalidRefreshInterval)?;
        Self::try_from(ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_interval_in_range_should_accept_it() {
        assert_eq!(" 500 ".parse(), Ok(RefreshInterval(500)));
        assert_eq!("3600000".parse(), Ok(RefreshInterval(3_600_000)));
    }

    #[test]
    fn given_interval_out_of_range_or_not_a_number_should_refuse_it() {
        for text in ["499", "3600001", "-1", "1.5", "soon", ""] {
            assert_eq!(
                text.parse::<RefreshInterval>(),
                Err(InvalidRefreshInterval),
                "{text}"
            );
        }
    }
}
