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

use std::{
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

use super::{BoxObject, Object, ObjectLayout, ObjectType};
use crate::elements::{array::Array, BoxElement, Element};

#[repr(C)]
#[derive(Default)]
pub struct RawString {
    ptr: Option<NonNull<Element<Array>>>,
}

impl Deref for RawString {
    type Target = Array;

    fn deref(&self) -> &Self::Target {
        unsafe { self.ptr.as_ref().expect("value MUST exists").as_ref() }
    }
}

impl DerefMut for RawString {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.ptr.as_mut().expect("value MUST exists").as_mut() }
    }
}

impl Drop for RawString {
    fn drop(&mut self) {
        unsafe {
            if let Some(ptr) = self.ptr.take() {
                BoxElement::from_raw(ptr);
            }
        }
    }
}

impl ObjectLayout for RawString {
    fn object_type() -> u16 {
        ObjectType::RAW_STRING.bits
    }
}

impl Object<RawString> {
    #[inline]
    pub fn has_value(&self) -> bool {
        self.value.ptr.is_some()
    }

    pub fn update_value(&mut self, value: Option<BoxElement<Array>>) -> Option<BoxElement<Array>> {
        if let Some(mut value) = value {
            value.associated_with(&self.meta);
            if let Some(old_value) = self.value.ptr.replace(BoxElement::leak(value)) {
                return unsafe { Some(BoxElement::from_raw(old_value)) };
            }
        } else if let Some(old_value) = self.value.ptr.take() {
            return unsafe { Some(BoxElement::from_raw(old_value)) };
        }
        None
    }
}

impl BoxObject<RawString> {
    pub fn with_key_value(key: &[u8], value: BoxElement<Array>) -> BoxObject<RawString> {
        let mut object: BoxObject<RawString> = BoxObject::with_key(key);
        object.update_value(Some(value));
        object
    }
}