use crate::agentic::tools::framework::ToolUseContext;
use crate::util::errors::{BitFunError, BitFunResult};

pub use crate::agentic::tools::workspace_paths::{
    normalize_path, resolve_path, resolve_path_with_workspace,
};

pub(crate) fn primary_api_format(context: &ToolUseContext) -> String {
    context.primary_model_facts().api_format.to_lowercase()
}

pub(crate) fn require_multimodal_tool_output(
    tool_name: &str,
    context: &ToolUseContext,
) -> BitFunResult<()> {
    if !context.primary_model_supports_image_understanding() {
        return Err(BitFunError::tool(format!(
            "{} is not allowed because the primary model does not accept image inputs",
            tool_name
        )));
    }

    if context
        .primary_model_facts()
        .multimodal_tool_output_supported()
    {
        return Ok(());
    }

    Err(BitFunError::tool(format!(
        "{} returns images in tool results; set the primary model to Anthropic (Claude) or OpenAI-compatible API format. Other providers are not supported yet.",
        tool_name
    )))
}

#[cfg(feature = "tools-image-analysis")]
pub(crate) fn supports_multimodal_tool_output(context: &ToolUseContext) -> bool {
    context.primary_model_supports_image_understanding()
        && context
            .primary_model_facts()
            .multimodal_tool_output_supported()
}

pub(crate) fn supported_image_mime_type(bytes: &[u8]) -> Option<&'static str> {
    match image::guess_format(bytes).ok()? {
        image::ImageFormat::Png => Some("image/png"),
        image::ImageFormat::Jpeg => Some("image/jpeg"),
        image::ImageFormat::Gif => Some("image/gif"),
        image::ImageFormat::WebP => Some("image/webp"),
        image::ImageFormat::Bmp => Some("image/bmp"),
        _ => None,
    }
}
