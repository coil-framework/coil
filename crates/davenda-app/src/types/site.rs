use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppSite {
    pub id: SiteId,
    pub display_name: String,
    pub brand_name: Option<String>,
    pub domains: Vec<AppDomain>,
    pub default_locale: LocaleTag,
    pub supported_locales: Vec<LocaleTag>,
}

impl AppSite {
    pub fn new(
        id: impl Into<String>,
        display_name: impl Into<String>,
        default_locale: LocaleTag,
        supported_locales: Vec<LocaleTag>,
    ) -> Result<Self, AppModelError> {
        Ok(Self {
            id: SiteId::new(id.into())?,
            display_name: require_non_empty("site_display_name", display_name.into())?,
            brand_name: None,
            domains: Vec::new(),
            default_locale,
            supported_locales,
        })
    }

    pub fn with_brand_name(mut self, brand_name: impl Into<String>) -> Result<Self, AppModelError> {
        self.brand_name = Some(require_non_empty("site_brand_name", brand_name.into())?);
        Ok(self)
    }

    pub fn with_domain(mut self, domain: AppDomain) -> Self {
        self.domains.push(domain);
        self
    }

    pub fn canonical_domain(&self) -> Option<&str> {
        self.domains
            .iter()
            .find(|domain| domain.canonical)
            .map(|domain| domain.hostname.as_str())
    }
}
