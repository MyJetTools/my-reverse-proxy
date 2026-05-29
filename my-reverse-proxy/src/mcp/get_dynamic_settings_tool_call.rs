use mcp_server_middleware::*;
use serde::*;

use crate::settings::SettingsModel;

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct GetDynamicSettingsInputData {}

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct GetDynamicSettingsResponse {
    #[property(description = "Resolved path of the dynamic settings file (from `dynamic_settings_file` in the main settings).")]
    pub path: String,

    #[property(description = "True if the dynamic settings file currently exists on disk. When false, `content` is empty.")]
    pub exists: bool,

    #[property(description = "Raw YAML content of the dynamic settings file, or an empty string if it does not exist yet.")]
    pub content: String,
}

pub struct GetDynamicSettingsHandler;

impl ToolDefinition for GetDynamicSettingsHandler {
    const FUNC_NAME: &'static str = "get_dynamic_settings";
    const DESCRIPTION: &'static str = "Read the raw YAML of the dynamic settings file — the single settings file pointed to by `dynamic_settings_file` that is editable over MCP. It is merged into the running configuration exactly like an `include:` file. Returns its resolved path and current content (empty if it does not exist yet). ALWAYS call this BEFORE set_dynamic_settings: edit the content this returns and write it back, instead of composing a new file from scratch — set_dynamic_settings is a full-file replace, so anything not present in what you send is lost. After writing, call reload_settings to apply.";
}

#[async_trait::async_trait]
impl McpToolCall<GetDynamicSettingsInputData, GetDynamicSettingsResponse> for GetDynamicSettingsHandler {
    async fn execute_tool_call(
        &self,
        _model: GetDynamicSettingsInputData,
    ) -> Result<GetDynamicSettingsResponse, String> {
        let path = resolve_dynamic_settings_file_path().await?;

        let (exists, content) = match tokio::fs::read(path.as_str()).await {
            Ok(bytes) => (true, String::from_utf8_lossy(&bytes).to_string()),
            Err(_) => (false, String::new()),
        };

        Ok(GetDynamicSettingsResponse {
            path,
            exists,
            content,
        })
    }
}

/// Loads the main settings file and resolves the configured `dynamic_settings_file` path.
/// Errors if the field is not configured.
pub async fn resolve_dynamic_settings_file_path() -> Result<String, String> {
    let settings_model = SettingsModel::load_async(None).await?;

    let dynamic_file = settings_model.dynamic_settings_file.ok_or_else(|| {
        "dynamic_settings_file is not configured in the main settings".to_string()
    })?;

    Ok(rust_extensions::file_utils::format_path(dynamic_file.as_str()).to_string())
}
