use mcp_server_middleware::*;
use serde::*;

use crate::settings::SettingsModel;

use super::resolve_dynamic_settings_file_path;

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct SetDynamicSettingsInputData {
    #[property(description = "Full YAML body to write to the dynamic settings file. It must parse as a valid settings document (same schema as ~/.my-reverse-proxy). The previous content is fully replaced.")]
    pub content: String,
}

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct SetDynamicSettingsResponse {
    #[property(description = "Resolved path of the dynamic settings file that was written.")]
    pub path: String,

    #[property(description = "Number of bytes written to the file.")]
    pub bytes_written: i64,
}

pub struct SetDynamicSettingsHandler;

impl ToolDefinition for SetDynamicSettingsHandler {
    const FUNC_NAME: &'static str = "set_dynamic_settings";
    const DESCRIPTION: &'static str = "Overwrite the dynamic settings file (the single file pointed to by `dynamic_settings_file`, the only settings file editable over MCP) with the provided YAML. IMPORTANT WORKFLOW — ALWAYS call get_dynamic_settings FIRST to read the current content, then edit that exact content and write the full result back. NEVER regenerate the file from scratch or from memory: doing so silently drops whatever was already there. This is a full-file replace, so the `content` you send must already contain everything you want to keep plus your changes. The content is validated against the settings schema before writing — on a parse error nothing is written. This only writes the file; call reload_settings afterwards to apply the change to the running proxy.";
}

#[async_trait::async_trait]
impl McpToolCall<SetDynamicSettingsInputData, SetDynamicSettingsResponse> for SetDynamicSettingsHandler {
    async fn execute_tool_call(
        &self,
        model: SetDynamicSettingsInputData,
    ) -> Result<SetDynamicSettingsResponse, String> {
        let path = resolve_dynamic_settings_file_path().await?;

        // Validate before writing — refuse to persist a file that won't parse.
        if let Err(err) =
            my_settings_reader::serde_yaml::from_slice::<SettingsModel>(model.content.as_bytes())
        {
            return Err(format!("Invalid settings YAML, nothing written. Err: {}", err));
        }

        let bytes_written = model.content.len() as i64;

        tokio::fs::write(path.as_str(), model.content.as_bytes())
            .await
            .map_err(|err| format!("Failed to write dynamic settings file {}: {}", path, err))?;

        Ok(SetDynamicSettingsResponse {
            path,
            bytes_written,
        })
    }
}
