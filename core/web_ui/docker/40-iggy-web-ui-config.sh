#!/bin/sh
# Licensed to the Apache Software Foundation (ASF) under one
# or more contributor license agreements.  See the NOTICE file
# distributed with this work for additional information
# regarding copyright ownership.  The ASF licenses this file
# to you under the Apache License, Version 2.0 (the
# "License"); you may not use this file except in compliance
# with the License.  You may obtain a copy of the License at
#
#   http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing,
# software distributed under the License is distributed on an
# "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
# KIND, either express or implied.  See the License for the
# specific language governing permissions and limitations
# under the License.

# Writes the UI's runtime config from PUBLIC_IGGY_API_URL, which the Helm
# chart sets. Unset or empty leaves the UI calling its own origin.

set -eu

config=/usr/share/nginx/html/ui/config.json
api_url="${PUBLIC_IGGY_API_URL:-}"
# URLs only need backslashes and quotes escaped to sit in a JSON string.
escaped=$(printf '%s' "$api_url" | sed 's/\\/\\\\/g; s/"/\\"/g')
printf '{"api_url":"%s"}\n' "$escaped" > "$config"
echo "iggy-web-ui: API URL is ${api_url:-the serving origin}"
