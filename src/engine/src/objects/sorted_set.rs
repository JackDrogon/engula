// Copyright 2022 The Engula Authors.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::collections::{BTreeMap, HashMap};

use super::{ObjectLayout, ObjectType};

#[repr(C)]
pub struct SortedSet {
    scores: BTreeMap<f64, String>,
    values: HashMap<String, f64>,
}

impl SortedSet {
    pub fn iter(&self) -> impl Iterator<Item = (&String, &f64)> {
        self.values.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&String, &mut f64)> {
        self.values.iter_mut()
    }
}

impl ObjectLayout for SortedSet {
    fn object_type() -> u16 {
        ObjectType::SORTED_SET.bits
    }
}