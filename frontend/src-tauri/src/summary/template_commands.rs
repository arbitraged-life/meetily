use crate::summary::templates;
use serde::{Deserialize, Serialize};
use tauri::Runtime;
use tracing::{info, warn};

/// Template metadata for UI display
#[derive(Debug, Serialize, Deserialize)]
pub struct TemplateInfo {
    /// Template identifier (e.g., "daily_standup", "standard_meeting")
    pub id: String,

    /// Display name for the template
    pub name: String,

    /// Brief description of the template's purpose
    pub description: String,
}

/// Detailed template structure for preview/debugging
#[derive(Debug, Serialize, Deserialize)]
pub struct TemplateDetails {
    /// Template identifier
    pub id: String,

    /// Display name
    pub name: String,

    /// Description
    pub description: String,

    /// List of section titles in order
    pub sections: Vec<String>,
}

/// Full template JSON response with source metadata
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateJsonResponse {
    pub id: String,
    pub name: String,
    pub description: String,
    pub is_custom: bool,
    pub source: String, // "custom", "bundled", or "builtIn"
    pub template_json: String,
}

/// Determine the source of a template and return TemplateJsonResponse
fn build_template_json_response(template_id: &str) -> Result<TemplateJsonResponse, String> {
    let custom_dir = templates::get_custom_templates_dir();
    let custom_path = custom_dir
        .as_ref()
        .map(|d| d.join(format!("{}.json", template_id)));

    // Check custom dir
    if let Some(ref path) = custom_path {
        if path.exists() {
            let json = std::fs::read_to_string(path)
                .map_err(|e| format!("Failed to read custom template: {}", e))?;
            let template = templates::validate_and_parse_template(&json)?;
            let pretty = serde_json::to_string_pretty(
                &serde_json::from_str::<serde_json::Value>(&json)
                    .map_err(|e| format!("Failed to parse JSON: {}", e))?,
            )
            .map_err(|e| format!("Failed to pretty-print JSON: {}", e))?;
            return Ok(TemplateJsonResponse {
                id: template_id.to_string(),
                name: template.name,
                description: template.description,
                is_custom: true,
                source: "custom".to_string(),
                template_json: pretty,
            });
        }
    }

    // Check if it's a builtin template
    let is_builtin = templates::get_builtin_template(template_id).is_some();

    // Try loading via get_template (which checks bundled before builtin)
    let template = templates::get_template(template_id)?;

    let value = serde_json::json!({
        "name": template.name,
        "description": template.description,
        "sections": template.sections,
    });
    let pretty = serde_json::to_string_pretty(&value)
        .map_err(|e| format!("Failed to serialize template: {}", e))?;

    let source = if is_builtin {
        "builtIn".to_string()
    } else {
        "bundled".to_string()
    };

    Ok(TemplateJsonResponse {
        id: template_id.to_string(),
        name: template.name,
        description: template.description,
        is_custom: false,
        source,
        template_json: pretty,
    })
}

/// Get the custom templates directory, creating it if needed
fn ensure_custom_templates_dir() -> Result<std::path::PathBuf, String> {
    let dir = templates::get_custom_templates_dir()
        .ok_or_else(|| "Could not determine custom templates directory".to_string())?;
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create custom templates directory: {}", e))?;
    Ok(dir)
}

/// Derive a template ID from a name
fn name_to_id(name: &str) -> String {
    let base: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c == ' ' { '_' } else { c })
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    base
}

/// Check if a template ID already exists
fn template_id_exists(id: &str) -> bool {
    templates::list_template_ids().contains(&id.to_string())
}

/// Find a unique template ID by appending _1, _2, etc.
fn unique_template_id(base_id: &str) -> String {
    if !template_id_exists(base_id) {
        return base_id.to_string();
    }
    let mut n = 1u32;
    loop {
        let candidate = format!("{}_{}", base_id, n);
        if !template_id_exists(&candidate) {
            return candidate;
        }
        n += 1;
    }
}

/// Save JSON to custom templates dir under given id
fn save_to_custom_dir(template_id: &str, json: &str) -> Result<(), String> {
    let dir = ensure_custom_templates_dir()?;
    let path = dir.join(format!("{}.json", template_id));
    std::fs::write(&path, json)
        .map_err(|e| format!("Failed to write template file: {}", e))
}


/// Lists all available templates
///
/// Returns templates from both built-in (embedded) and custom (user data directory) sources.
/// Templates are automatically discovered - no code changes needed to add new templates.
///
/// # Returns
/// Vector of TemplateInfo with id, name, and description for each template
#[tauri::command]
pub async fn api_list_templates<R: Runtime>(
    _app: tauri::AppHandle<R>,
) -> Result<Vec<TemplateInfo>, String> {
    info!("api_list_templates called");

    let templates = templates::list_templates();

    let template_infos: Vec<TemplateInfo> = templates
        .into_iter()
        .map(|(id, name, description)| TemplateInfo {
            id,
            name,
            description,
        })
        .collect();

    info!("Found {} available templates", template_infos.len());

    Ok(template_infos)
}

/// Gets detailed information about a specific template
///
/// # Arguments
/// * `template_id` - Template identifier (e.g., "daily_standup")
///
/// # Returns
/// TemplateDetails with full template structure
#[tauri::command]
pub async fn api_get_template_details<R: Runtime>(
    _app: tauri::AppHandle<R>,
    template_id: String,
) -> Result<TemplateDetails, String> {
    info!("api_get_template_details called for template_id: {}", template_id);

    let template = templates::get_template(&template_id)?;

    let section_titles: Vec<String> = template
        .sections
        .iter()
        .map(|section| section.title.clone())
        .collect();

    let details = TemplateDetails {
        id: template_id,
        name: template.name,
        description: template.description,
        sections: section_titles,
    };

    info!("Retrieved template details for '{}'", details.name);

    Ok(details)
}

/// Validates a custom template JSON string
///
/// Useful for template editor UI or validation before saving custom templates
///
/// # Arguments
/// * `template_json` - Raw JSON string of the template
///
/// # Returns
/// Ok(template_name) if valid, Err(error_message) if invalid
#[tauri::command]
pub async fn api_validate_template<R: Runtime>(
    _app: tauri::AppHandle<R>,
    template_json: String,
) -> Result<String, String> {
    info!("api_validate_template called");

    match templates::validate_and_parse_template(&template_json) {
        Ok(template) => {
            info!("Template '{}' validated successfully", template.name);
            Ok(template.name)
        }
        Err(e) => {
            warn!("Template validation failed: {}", e);
            Err(e)
        }
    }
}

/// Returns the full JSON content of a template along with metadata
#[tauri::command]
pub async fn api_get_template_json<R: Runtime>(
    _app: tauri::AppHandle<R>,
    template_id: String,
) -> Result<TemplateJsonResponse, String> {
    info!("api_get_template_json called for template_id: {}", template_id);
    build_template_json_response(&template_id)
}

/// Saves (creates or overwrites) a template in the custom templates directory
#[tauri::command]
pub async fn api_save_template<R: Runtime>(
    _app: tauri::AppHandle<R>,
    template_id: String,
    template_json: String,
) -> Result<TemplateJsonResponse, String> {
    info!("api_save_template called for template_id: {}", template_id);

    // Validate first
    templates::validate_and_parse_template(&template_json)?;

    // Pretty-print before saving
    let pretty = serde_json::to_string_pretty(
        &serde_json::from_str::<serde_json::Value>(&template_json)
            .map_err(|e| format!("Failed to parse JSON: {}", e))?,
    )
    .map_err(|e| format!("Failed to pretty-print JSON: {}", e))?;

    save_to_custom_dir(&template_id, &pretty)?;

    info!("Saved template '{}' to custom dir", template_id);
    build_template_json_response(&template_id)
}

/// Resets a template by deleting the custom override, reverting to bundled/builtin
#[tauri::command]
pub async fn api_reset_template<R: Runtime>(
    _app: tauri::AppHandle<R>,
    template_id: String,
) -> Result<TemplateJsonResponse, String> {
    info!("api_reset_template called for template_id: {}", template_id);

    if let Some(custom_dir) = templates::get_custom_templates_dir() {
        let path = custom_dir.join(format!("{}.json", template_id));
        if path.exists() {
            std::fs::remove_file(&path)
                .map_err(|e| format!("Failed to delete custom template: {}", e))?;
            info!("Deleted custom override for '{}'", template_id);
        }
    }

    build_template_json_response(&template_id)
}

/// Creates a new custom template from JSON
///
/// Derives the template ID from the name field.
#[tauri::command]
pub async fn api_create_template<R: Runtime>(
    _app: tauri::AppHandle<R>,
    template_json: String,
) -> Result<TemplateJsonResponse, String> {
    info!("api_create_template called");

    let template = templates::validate_and_parse_template(&template_json)?;
    let base_id = name_to_id(&template.name);
    let template_id = unique_template_id(&base_id);

    let pretty = serde_json::to_string_pretty(
        &serde_json::from_str::<serde_json::Value>(&template_json)
            .map_err(|e| format!("Failed to parse JSON: {}", e))?,
    )
    .map_err(|e| format!("Failed to pretty-print JSON: {}", e))?;

    save_to_custom_dir(&template_id, &pretty)?;

    info!("Created new template '{}' as id '{}'", template.name, template_id);
    build_template_json_response(&template_id)
}

/// Deletes a custom template (only custom templates can be deleted)
#[tauri::command]
pub async fn api_delete_template<R: Runtime>(
    _app: tauri::AppHandle<R>,
    template_id: String,
) -> Result<(), String> {
    info!("api_delete_template called for template_id: {}", template_id);

    let custom_dir = templates::get_custom_templates_dir()
        .ok_or_else(|| "Could not determine custom templates directory".to_string())?;
    let path = custom_dir.join(format!("{}.json", template_id));

    if !path.exists() {
        return Err(format!(
            "Template '{}' is not a custom template and cannot be deleted",
            template_id
        ));
    }

    std::fs::remove_file(&path)
        .map_err(|e| format!("Failed to delete template: {}", e))?;

    info!("Deleted custom template '{}'", template_id);
    Ok(())
}

/// Duplicates an existing template as a new custom template
#[tauri::command]
pub async fn api_duplicate_template<R: Runtime>(
    _app: tauri::AppHandle<R>,
    template_id: String,
) -> Result<TemplateJsonResponse, String> {
    info!("api_duplicate_template called for template_id: {}", template_id);

    let template = templates::get_template(&template_id)?;
    let copy_name = format!("{} (Copy)", template.name);
    let base_copy_id = format!("{}_copy", template_id);
    let new_id = unique_template_id(&base_copy_id);

    let value = serde_json::json!({
        "name": copy_name,
        "description": template.description,
        "sections": template.sections,
    });
    let pretty = serde_json::to_string_pretty(&value)
        .map_err(|e| format!("Failed to serialize template: {}", e))?;

    save_to_custom_dir(&new_id, &pretty)?;

    info!("Duplicated template '{}' as '{}'", template_id, new_id);
    build_template_json_response(&new_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_list_templates() {
        // This test requires the templates to be embedded/available
        // In a real test environment, you might want to mock the templates module

        // For now, just verify the function compiles and runs
        // You can expand this with more specific assertions
    }

    #[tokio::test]
    async fn test_validate_template_valid() {
        let valid_json = r#"
        {
            "name": "Test Template",
            "description": "A test template",
            "sections": [
                {
                    "title": "Summary",
                    "instruction": "Provide a summary",
                    "format": "paragraph"
                }
            ]
        }"#;

        // Mock app handle would be needed for actual testing
        // For now, test the validation logic directly
        let result = templates::validate_and_parse_template(valid_json);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_validate_template_invalid() {
        let invalid_json = "invalid json";

        let result = templates::validate_and_parse_template(invalid_json);
        assert!(result.is_err());
    }
}
