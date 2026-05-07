use crate::settings::{
    AddHeaderSettingsModel, AddHttpHeadersSettings, ModifyHttpHeadersSettings,
    RemoveHttpHeadersSettings,
};

use super::VariablesCompiled;

pub fn populate_modify_http_headers_settings(
    modify_http_headers: Option<ModifyHttpHeadersSettings>,
    variables: &VariablesCompiled,
) -> Result<Option<ModifyHttpHeadersSettings>, String> {
    if modify_http_headers.is_none() {
        return Ok(None);
    }

    let src = modify_http_headers.unwrap();

    if src.add.is_none() && src.remove.is_none() {
        return Ok(None);
    }

    let mut result = ModifyHttpHeadersSettings::default();

    if let Some(add) = src.add {
        let request = populate_add_header(add.request, variables)?;
        let response = populate_add_header(add.response, variables)?;
        result.add = Some(AddHttpHeadersSettings { request, response });
    }

    if let Some(remove) = src.remove {
        let request = populate_vec_of_string_opt(remove.request, variables)?;
        let response = populate_vec_of_string_opt(remove.response, variables)?;
        result.remove = Some(RemoveHttpHeadersSettings { request, response });
    }

    Ok(Some(result))
}

fn populate_add_header(
    src: Option<Vec<AddHeaderSettingsModel>>,
    variables: &VariablesCompiled,
) -> Result<Option<Vec<AddHeaderSettingsModel>>, String> {
    if let Some(request) = src {
        let mut result = Vec::with_capacity(request.len());
        for itm in request {
            result.push(AddHeaderSettingsModel {
                name: variables.apply_variables(itm.name)?,
                value: variables.apply_variables(itm.value)?,
            });
        }

        return Ok(Some(result));
    }

    Ok(None)
}

pub fn populate_vec_of_string_opt(
    src: Option<Vec<String>>,
    variables: &VariablesCompiled,
) -> Result<Option<Vec<String>>, String> {
    if let Some(items) = src {
        let mut result = Vec::with_capacity(items.len());
        for itm in items {
            let itm = variables.apply_variables(itm)?;

            result.push(itm);
        }
        return Ok(Some(result));
    }

    Ok(None)
}

pub fn populate_vec_of_string(
    src: Vec<String>,
    variables: &VariablesCompiled,
) -> Result<Vec<String>, String> {
    let mut result = Vec::with_capacity(src.len());
    for itm in src {
        let itm = variables.apply_variables(itm)?;

        result.push(itm);
    }

    Ok(result)
}
