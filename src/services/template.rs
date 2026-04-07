use std::collections::HashMap;
use std::sync::Arc;
use tera::{Tera, Context, Value, Error as TeraError};
use tokio::sync::RwLock;
use rust_embed::RustEmbed;

use super::i18n::{I18n, get_i18n};

/// Embedded templates and assets
#[derive(RustEmbed)]
#[folder = "web/html"]
pub struct Templates;

/// Embedded assets (JS, CSS, etc.)
#[derive(RustEmbed)]
#[folder = "web/assets"]
pub struct Assets;

/// Template service for rendering HTML pages
#[derive(Clone)]
#[allow(dead_code)]
pub struct TemplateService {
    tera: Arc<RwLock<Tera>>,
    i18n: I18n,
}

impl TemplateService {
    /// Create a new template service
    pub fn new() -> Self {
        let tera = Self::create_tera().expect("Failed to create Tera instance");
        Self {
            tera: Arc::new(RwLock::new(tera)),
            i18n: get_i18n(),
        }
    }

    /// Create and configure Tera instance
    fn create_tera() -> anyhow::Result<Tera> {
        let mut tera = Tera::default();

        // Configure Tera with autoescape for html files
        tera.autoescape_on(vec!["html"]);

        // Load templates from embedded files
        Self::load_templates(&mut tera)?;

        // Register custom functions
        tera.register_function("i18n", Box::new(i18n_function));

        Ok(tera)
    }

    /// Load all templates from embedded files
    fn load_templates(tera: &mut Tera) -> anyhow::Result<()> {
        // Load templates in order - base templates first
        let templates = [
            ("base.html", include_str!("../../web/html/base.html")),
            ("login.html", include_str!("../../web/html/login.html")),
            ("index.html", include_str!("../../web/html/index.html")),
            ("inbounds.html", include_str!("../../web/html/inbounds.html")),
            ("settings.html", include_str!("../../web/html/settings.html")),
            ("xray.html", include_str!("../../web/html/xray.html")),
            // Components
            ("component/aSidebar.html", include_str!("../../web/html/component/aSidebar.html")),
            ("component/aThemeSwitch.html", include_str!("../../web/html/component/aThemeSwitch.html")),
            ("component/aClientTable.html", include_str!("../../web/html/component/aClientTable.html")),
            ("component/aCustomStatistic.html", include_str!("../../web/html/component/aCustomStatistic.html")),
            ("component/aSettingListItem.html", include_str!("../../web/html/component/aSettingListItem.html")),
            ("component/aTableSortable.html", include_str!("../../web/html/component/aTableSortable.html")),
            ("component/aPersianDatepicker.html", include_str!("../../web/html/component/aPersianDatepicker.html")),
            // Modals
            ("modals/inbound_modal.html", include_str!("../../web/html/modals/inbound_modal.html")),
            ("modals/client_modal.html", include_str!("../../web/html/modals/client_modal.html")),
            ("modals/qrcode_modal.html", include_str!("../../web/html/modals/qrcode_modal.html")),
            ("modals/text_modal.html", include_str!("../../web/html/modals/text_modal.html")),
            ("modals/prompt_modal.html", include_str!("../../web/html/modals/prompt_modal.html")),
            ("modals/two_factor_modal.html", include_str!("../../web/html/modals/two_factor_modal.html")),
            ("modals/inbound_info_modal.html", include_str!("../../web/html/modals/inbound_info_modal.html")),
            ("modals/client_bulk_modal.html", include_str!("../../web/html/modals/client_bulk_modal.html")),
            // Forms
            ("form/inbound.html", include_str!("../../web/html/form/inbound.html")),
            ("form/client.html", include_str!("../../web/html/form/client.html")),
            ("form/tls_settings.html", include_str!("../../web/html/form/tls_settings.html")),
            ("form/reality_settings.html", include_str!("../../web/html/form/reality_settings.html")),
            ("form/sniffing.html", include_str!("../../web/html/form/sniffing.html")),
            // Settings
            ("settings/panel/general.html", include_str!("../../web/html/settings/panel/general.html")),
            ("settings/panel/security.html", include_str!("../../web/html/settings/panel/security.html")),
            ("settings/panel/telegram.html", include_str!("../../web/html/settings/panel/telegram.html")),
            ("settings/xray/basics.html", include_str!("../../web/html/settings/xray/basics.html")),
            ("settings/xray/advanced.html", include_str!("../../web/html/settings/xray/advanced.html")),
        ];

        for (name, content) in templates {
            tera.add_raw_template(name, content)?;
        }

        Ok(())
    }

    /// Render a template with the given context
    pub async fn render(&self, template: &str, context: &Context) -> Result<String, TeraError> {
        let tera = self.tera.read().await;
        tera.render(template, context)
    }

    /// Render login page
    pub async fn render_login(&self, base_path: &str, host: &str, lang: &str) -> Result<String, TeraError> {
        let mut context = Context::new();
        context.insert("base_path", &format!("{}panel/", base_path));
        context.insert("host", host);
        context.insert("title", "pages.login.title");
        context.insert("cur_ver", &env!("CARGO_PKG_VERSION"));
        context.insert("lang", lang);

        self.render("login.html", &context).await
    }

    /// Render index/dashboard page
    pub async fn render_index(&self, base_path: &str, host: &str, lang: &str) -> Result<String, TeraError> {
        let mut context = Context::new();
        context.insert("base_path", &format!("{}panel/", base_path));
        context.insert("host", host);
        context.insert("title", "pages.index.title");
        context.insert("cur_ver", &env!("CARGO_PKG_VERSION"));
        context.insert("lang", lang);

        self.render("index.html", &context).await
    }

    /// Render inbounds page
    pub async fn render_inbounds(&self, base_path: &str, host: &str, lang: &str) -> Result<String, TeraError> {
        let mut context = Context::new();
        context.insert("base_path", &format!("{}panel/", base_path));
        context.insert("host", host);
        context.insert("title", "pages.inbounds.title");
        context.insert("cur_ver", &env!("CARGO_PKG_VERSION"));
        context.insert("lang", lang);

        self.render("inbounds.html", &context).await
    }

    /// Render settings page
    pub async fn render_settings(&self, base_path: &str, host: &str, lang: &str) -> Result<String, TeraError> {
        let mut context = Context::new();
        context.insert("base_path", &format!("{}panel/", base_path));
        context.insert("host", host);
        context.insert("title", "pages.settings.title");
        context.insert("cur_ver", &env!("CARGO_PKG_VERSION"));
        context.insert("lang", lang);

        self.render("settings.html", &context).await
    }

    /// Render xray config page
    pub async fn render_xray(&self, base_path: &str, host: &str, lang: &str) -> Result<String, TeraError> {
        let mut context = Context::new();
        context.insert("base_path", &format!("{}panel/", base_path));
        context.insert("host", host);
        context.insert("title", "pages.xray.title");
        context.insert("cur_ver", &env!("CARGO_PKG_VERSION"));
        context.insert("lang", lang);

        self.render("xray.html", &context).await
    }

    /// Reload templates (for development)
    pub async fn reload(&self) -> anyhow::Result<()> {
        let mut tera = self.tera.write().await;
        let new_tera = Self::create_tera()?;
        *tera = new_tera;
        Ok(())
    }
}

impl Default for TemplateService {
    fn default() -> Self {
        Self::new()
    }
}

/// i18n template function for Tera
fn i18n_function(args: &HashMap<String, Value>) -> Result<Value, TeraError> {
    let key_value = args.get("key")
        .ok_or_else(|| TeraError::msg("i18n function requires 'key' argument"))?;
    let key = key_value.as_str()
        .ok_or_else(|| TeraError::msg("key must be a string"))?;

    let lang = args.get("lang")
        .and_then(|v| v.as_str())
        .unwrap_or("en_US");

    // Get translation - this is synchronous for template rendering
    let translation = get_translation_sync(key, lang);

    Ok(Value::String(translation))
}

/// Synchronous translation lookup (cached)
fn get_translation_sync(key: &str, lang: &str) -> String {
    // Load translations on first use
    use std::sync::OnceLock;
    static TRANSLATIONS: OnceLock<HashMap<String, HashMap<String, String>>> = OnceLock::new();

    let translations = TRANSLATIONS.get_or_init(|| {
        load_all_translations()
    });

    // Try requested language first
    if let Some(lang_map) = translations.get(lang) {
        if let Some(value) = lang_map.get(key) {
            return value.clone();
        }
    }

    // Fallback to English
    if let Some(lang_map) = translations.get("en_US") {
        if let Some(value) = lang_map.get(key) {
            return value.clone();
        }
    }

    // Return key if not found
    key.to_string()
}

/// Load all translations at startup
fn load_all_translations() -> HashMap<String, HashMap<String, String>> {
    let mut result = HashMap::new();

    let langs = [
        ("en_US", include_str!("../../web/translation/translate.en_US.toml")),
        ("zh_CN", include_str!("../../web/translation/translate.zh_CN.toml")),
        ("zh_TW", include_str!("../../web/translation/translate.zh_TW.toml")),
        ("ru_RU", include_str!("../../web/translation/translate.ru_RU.toml")),
        ("vi_VN", include_str!("../../web/translation/translate.vi_VN.toml")),
        ("fa_IR", include_str!("../../web/translation/translate.fa_IR.toml")),
        ("ar_EG", include_str!("../../web/translation/translate.ar_EG.toml")),
        ("es_ES", include_str!("../../web/translation/translate.es_ES.toml")),
        ("ja_JP", include_str!("../../web/translation/translate.ja_JP.toml")),
        ("id_ID", include_str!("../../web/translation/translate.id_ID.toml")),
        ("pt_BR", include_str!("../../web/translation/translate.pt_BR.toml")),
        ("tr_TR", include_str!("../../web/translation/translate.tr_TR.toml")),
        ("uk_UA", include_str!("../../web/translation/translate.uk_UA.toml")),
    ];

    for (lang, content) in langs {
        if let Ok(map) = parse_toml_translations(content) {
            result.insert(lang.to_string(), map);
        }
    }

    result
}

/// Parse TOML translation file into a flat key-value map
fn parse_toml_translations(content: &str) -> anyhow::Result<HashMap<String, String>> {
    let mut map = HashMap::new();
    let value: toml::Value = toml::from_str(content)?;
    flatten_toml(&value, "", &mut map);
    Ok(map)
}

/// Recursively flatten TOML structure into dot-notation keys
fn flatten_toml(value: &toml::Value, prefix: &str, map: &mut HashMap<String, String>) {
    match value {
        toml::Value::Table(table) => {
            for (key, val) in table {
                let new_prefix = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{}.{}", prefix, key)
                };
                flatten_toml(val, &new_prefix, map);
            }
        }
        toml::Value::String(s) => {
            map.insert(prefix.to_string(), s.clone());
        }
        toml::Value::Integer(i) => {
            map.insert(prefix.to_string(), i.to_string());
        }
        toml::Value::Boolean(b) => {
            map.insert(prefix.to_string(), b.to_string());
        }
        _ => {}
    }
}

/// Get asset content by path
pub fn get_asset(path: &str) -> Option<Vec<u8>> {
    Assets::get(path).map(|f| f.data.to_vec())
}

/// Get content type for asset
pub fn get_content_type(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("");
    match ext {
        "js" => "application/javascript",
        "css" => "text/css",
        "html" => "text/html",
        "json" => "application/json",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        "woff" | "woff2" => "font/woff2",
        "map" => "application/json",
        _ => "application/octet-stream",
    }
}
