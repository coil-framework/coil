use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheRuntimeServices {
    pub topology: CacheTopology,
    pub planner: CachePlanner,
}

impl CacheRuntimeServices {
    pub fn shared_invalidation_enabled(&self) -> bool {
        self.topology.supports_shared_invalidation()
    }

    pub fn distributed_backend(&self) -> Option<DistributedCacheBackend> {
        self.topology.l2()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmRuntimeServices {
    pub extension_directory: String,
    pub allow_network: bool,
    pub limits: WasmLimitsProfile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WasmLimitsProfile {
    pub page: ResourceLimits,
    pub api: ResourceLimits,
    pub job: ResourceLimits,
    pub scheduled_job: ResourceLimits,
    pub webhook: ResourceLimits,
    pub admin_widget: ResourceLimits,
    pub render_hook: ResourceLimits,
}

impl WasmLimitsProfile {
    pub fn for_point(&self, point: ExtensionPointKind) -> ResourceLimits {
        match point {
            ExtensionPointKind::Page => self.page,
            ExtensionPointKind::Api => self.api,
            ExtensionPointKind::Job => self.job,
            ExtensionPointKind::ScheduledJob => self.scheduled_job,
            ExtensionPointKind::Webhook => self.webhook,
            ExtensionPointKind::AdminWidget => self.admin_widget,
            ExtensionPointKind::RenderHook => self.render_hook,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateRuntimeServices {
    pub customer_app_namespace: TemplateNamespace,
    pub core_namespace: TemplateNamespace,
    pub registry: TemplateRegistry,
    pub runtime: TemplateRuntime,
}

impl TemplateRuntimeServices {
    pub fn namespace_chain(
        &self,
        module_namespace: Option<&TemplateNamespace>,
    ) -> Vec<TemplateNamespace> {
        let mut chain = vec![self.customer_app_namespace.clone()];

        if let Some(module_namespace) = module_namespace {
            if module_namespace != &self.customer_app_namespace
                && module_namespace != &self.core_namespace
            {
                chain.push(module_namespace.clone());
            }
        }

        chain.push(self.core_namespace.clone());
        chain
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct I18nRuntimeServices {
    pub default_locale: LocaleTag,
    pub supported_locales: Vec<LocaleTag>,
    pub fallback_locale: LocaleTag,
    pub router: LocaleRouter,
    pub translations: TranslationRuntime,
}

impl I18nRuntimeServices {
    pub fn request_context(&self, requested_locale: Option<&str>) -> LocaleContext {
        let resolved = requested_locale
            .and_then(|locale| {
                self.supported_locales
                    .iter()
                    .find(|candidate| candidate.as_str() == locale)
            })
            .cloned()
            .unwrap_or_else(|| self.default_locale.clone());

        LocaleContext::new(
            resolved.clone(),
            vec![self.fallback_locale.clone()],
            currency_for_locale(&resolved),
            timezone_for_locale(&resolved),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeoRuntimeServices {
    pub canonical_host: String,
    pub emit_json_ld: bool,
    pub sitemap_enabled: bool,
}

impl SeoRuntimeServices {
    pub fn allows_json_ld(&self) -> bool {
        self.emit_json_ld
    }

    pub fn can_emit_metadata(&self, metadata: &HeadMetadata) -> bool {
        !metadata.canonical_url.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct A11yRuntimeServices {
    pub navigation: NavigationContract,
    pub theme_baseline: ThemeAccessibilityContract,
}

pub type CliRuntimeServices = CliRuntime;
pub type DataRuntimeServices = DataRuntime;
pub type JobsRuntimeServices = JobsRuntime;
pub type ObservabilityRuntimeServices = ObservabilityRuntime;
pub type TlsRuntimeServices = TlsRuntime;

#[derive(Debug, Clone)]
pub struct CoreBootstrap {
    pub registry: ServiceRegistry,
    pub cache: CacheRuntimeServices,
    pub browser: BrowserSecurityServices,
    pub cli: CliRuntimeServices,
    pub data: DataRuntimeServices,
    pub jobs: JobsRuntimeServices,
    pub observability: ObservabilityRuntimeServices,
    pub i18n: I18nRuntimeServices,
    pub seo: SeoRuntimeServices,
    pub a11y: A11yRuntimeServices,
    pub template: TemplateRuntimeServices,
    pub tls: TlsRuntimeServices,
    pub wasm: WasmRuntimeServices,
}
