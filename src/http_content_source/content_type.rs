#[derive(Debug, Clone)]
pub enum WebContentType {
    Html,
    Css,
    Png,
    Svg,
    JavaScript,
    Json,
    Text,
    Raw(String),
}

impl WebContentType {
    pub fn as_str(&self) -> &str {
        match self {
            WebContentType::Html => "text/html",
            WebContentType::Css => "text/css",
            WebContentType::JavaScript => "text/javascript",
            WebContentType::Json => "application/json",
            WebContentType::Text => "text/plain; charset=utf-8",
            WebContentType::Png => "image/png",
            WebContentType::Svg => "image/svg+xml",
            WebContentType::Raw(content_type) => content_type.as_str(),
        }
    }

    pub fn detect_by_extension(path: &str) -> Option<Self> {
        let res = path.split(".");

        let el = res.last()?;

        match el.to_lowercase().as_str() {
            "png" => WebContentType::Png.into(),
            "svg" => WebContentType::Svg.into(),
            "css" => WebContentType::Css.into(),
            "js" => WebContentType::JavaScript.into(),
            "html" => WebContentType::Html.into(),
            "htm" => WebContentType::Html.into(),
            "text" => WebContentType::Text.into(),
            "json" => WebContentType::Json.into(),
            _ => None,
        }
    }
}

impl Into<WebContentType> for String {
    fn into(self) -> WebContentType {
        WebContentType::Raw(self)
    }
}
