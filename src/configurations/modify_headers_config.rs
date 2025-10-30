use std::collections::{HashMap, HashSet};

use crate::settings::{HttpEndpointModifyHeadersSettings, ModifyHttpHeadersSettings};

#[derive(Default)]
pub struct ModifyHeadersConfig {
    remove: HashSet<String>,
    add_headers: HashMap<String, String>,
}

impl ModifyHeadersConfig {
    pub fn populate_request(&mut self, settings: &mut ModifyHttpHeadersSettings) {
        if let Some(remove_settings) = settings.remove.as_mut() {
            if let Some(remove) = remove_settings.request.take() {
                for remove in remove {
                    self.remove.insert(remove);
                }
            }
        }

        if let Some(add) = settings.add.as_mut() {
            if let Some(add) = add.request.take() {
                for add in add {
                    self.add_headers.insert(add.name, add.value);
                }
            }
        }
    }

    pub fn populate_response(&mut self, settings: &mut ModifyHttpHeadersSettings) {
        if let Some(remove_settings) = settings.remove.as_mut() {
            if let Some(remove) = remove_settings.response.take() {
                for remove in remove {
                    self.remove.insert(remove);
                }
            }
        }

        if let Some(add) = settings.add.as_mut() {
            if let Some(add) = add.response.take() {
                for add in add {
                    self.add_headers.insert(add.name, add.value);
                }
            }
        }
    }

    pub fn new_request(src: &mut HttpEndpointModifyHeadersSettings) -> Self {
        let mut result = Self::default();
        if let Some(global) = src.global_modify_headers_settings.as_mut() {
            result.populate_request(global);
        }
        if let Some(end_point) = src.endpoint_modify_headers_settings.as_mut() {
            result.populate_request(end_point);
        }

        result
    }

    pub fn new_response(src: &mut HttpEndpointModifyHeadersSettings) -> Self {
        let mut result = Self::default();
        if let Some(global) = src.global_modify_headers_settings.as_mut() {
            result.populate_response(global);
        }
        if let Some(end_point) = src.endpoint_modify_headers_settings.as_mut() {
            result.populate_response(end_point);
        }

        result
    }

    pub fn iter_remove<'s>(&'s self) -> impl Iterator<Item = &'s String> {
        self.remove.iter()
    }

    pub fn iter_add<'s>(&'s self) -> impl Iterator<Item = (&'s String, &'s String)> {
        self.add_headers.iter()
    }

    pub fn has_to_be_removed(&self, header: &str) -> bool {
        for to_remove in self.remove.iter() {
            if to_remove.eq_ignore_ascii_case(header) {
                return true;
            }
        }

        for add in self.add_headers.keys() {
            if add.eq_ignore_ascii_case(header) {
                return true;
            }
        }

        false
    }
}
