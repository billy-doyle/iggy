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

//! JSON values in the browser's local storage. The browser can block site
//! storage, where reading `window.localStorage` throws, so every failure
//! reads as nothing stored and writes are best effort.

use serde::Serialize;
use serde::de::DeserializeOwned;
use web_sys::Storage;

/// `None` for a missing key, a value that does not parse or blocked storage.
pub fn read<T: DeserializeOwned>(key: &str) -> Option<T> {
    let stored = local_storage()?.get_item(key).ok()??;
    serde_json::from_str(&stored).ok()
}

pub fn write<T: Serialize>(key: &str, value: &T) {
    let (Some(storage), Ok(json)) = (local_storage(), serde_json::to_string(value)) else {
        return;
    };
    let _ = storage.set_item(key, &json);
}

pub fn remove(key: &str) {
    if let Some(storage) = local_storage() {
        let _ = storage.remove_item(key);
    }
}

fn local_storage() -> Option<Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}
