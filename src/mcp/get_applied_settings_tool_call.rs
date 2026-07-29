use mcp_server_middleware::*;
use serde::*;

use crate::app::APP_CTX;

use super::{
    build_settings_summary, load_current_configuration_errors, resolve_settings_file_path,
    GetSettingsResponse,
};

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct GetAppliedSettingsInputData {}

pub struct GetAppliedSettingsHandler;

impl ToolDefinition for GetAppliedSettingsHandler {
    const FUNC_NAME: &'static str = "get_applied_settings";
    const DESCRIPTION: &'static str = "Return the settings that are CURRENTLY APPLIED to the running proxy — a snapshot taken at the last successful reload, NOT a fresh compile of the files. Same shape as get_settings, so you can diff the two: get_settings shows what the files declare now, get_applied_settings shows what is actually live. A difference means there are pending changes that a reload_settings would apply. Errors if nothing has been applied yet.";
}

#[async_trait::async_trait]
impl McpToolCall<GetAppliedSettingsInputData, GetSettingsResponse> for GetAppliedSettingsHandler {
    async fn execute_tool_call(
        &self,
        _model: GetAppliedSettingsInputData,
    ) -> Result<GetSettingsResponse, String> {
        let applied = APP_CTX
            .applied_settings
            .load_full()
            .ok_or_else(|| "No settings have been applied yet".to_string())?;

        let current_configuration_errors = load_current_configuration_errors().await;

        Ok(build_settings_summary(
            applied.as_ref(),
            resolve_settings_file_path(),
            current_configuration_errors,
        ))
    }
}
