use super::*;
use davenda_template::TemplateModelError;

#[derive(Debug, Error)]
pub enum RuntimeBuildError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Registration(#[from] RegistrationError),
    #[error(transparent)]
    Capability(#[from] CapabilityValidationError),
    #[error(transparent)]
    ModuleInstallation(#[from] ModuleInstallationError),
    #[error(transparent)]
    Data(#[from] DataModelError),
    #[error(transparent)]
    Route(#[from] RouteBuildError),
    #[error(transparent)]
    Observability(#[from] ObservabilityError),
    #[error(transparent)]
    Wasm(#[from] WasmModelError),
    #[error(transparent)]
    Jobs(#[from] JobsModelError),
    #[error(transparent)]
    Ops(#[from] OpsModelError),
    #[error(transparent)]
    Template(#[from] TemplateModelError),
    #[error("failed to read template source `{path}`: {message}")]
    TemplateSourceRead { path: String, message: String },
    #[error("failed to parse template source `{path}`: {message}")]
    TemplateSourceParse { path: String, message: String },
    #[error("failed to read storefront catalog `{path}`: {message}")]
    StorefrontCatalogRead { path: String, message: String },
    #[error("failed to parse storefront catalog `{path}`: {message}")]
    StorefrontCatalogParse { path: String, message: String },
    #[error("storefront catalog `{path}` is invalid: {message}")]
    StorefrontCatalogValidation { path: String, message: String },
    #[error("template source `{path}` uses unsupported directive `{directive}`")]
    TemplateSourceUnsupportedDirective { path: String, directive: String },
    #[error("customer app root `{path}` does not exist")]
    MissingCustomerAppRoot { path: String },
    #[error("customer app root `{path}` is not a directory")]
    CustomerAppRootNotDirectory { path: String },
    #[error("customer app templates directory `{path}` does not exist")]
    MissingTemplateTree { path: String },
    #[error("customer app templates directory `{path}` does not contain any `.html` templates")]
    EmptyTemplateTree { path: String },
    #[error("configured auth package `{configured}` does not match loaded package `{actual}`")]
    AuthPackageMismatch { configured: String, actual: String },
    #[error("linked customer plugin `{plugin_id}` is registered more than once")]
    DuplicateCustomerPlugin { plugin_id: String },
    #[error("failed to register linked customer plugin `{plugin_id}`: {message}")]
    CustomerPluginRegistration { plugin_id: String, message: String },
    #[error(
        "installed extension `{extension_id}` targets customer app `{actual}` but runtime config is `{configured}`"
    )]
    ExtensionCustomerAppMismatch {
        extension_id: String,
        configured: String,
        actual: String,
    },
    #[error("handler `{route}` is registered more than once")]
    DuplicateHandler { route: String },
    #[error("handler `{route}` does not match a registered route")]
    UnknownHandlerRoute { route: String },
    #[error(
        "extension slot `{surface}` for `{kind:?}` is declared by both `{first_module}` and `{second_module}`"
    )]
    DuplicateExtensionSlot {
        kind: ExtensionPointKind,
        surface: String,
        first_module: String,
        second_module: String,
    },
    #[error(
        "installed extension `{extension_id}` handler `{handler_id}` targets `{point}` surface `{surface}` without a declared slot"
    )]
    UnknownExtensionSlot {
        extension_id: String,
        handler_id: String,
        point: ExtensionPointKind,
        surface: String,
    },
    #[error(
        "job `{job}` is declared by both `{first_module}` and `{second_module}`; runtime job names must be unique"
    )]
    DuplicateRuntimeJobName {
        job: String,
        first_module: String,
        second_module: String,
    },
    #[error(
        "runtime data repository `{repository}` is declared by both `{first_module}` and `{second_module}`"
    )]
    DuplicateDataRepository {
        repository: String,
        first_module: String,
        second_module: String,
    },
    #[error("event subscription `{event}` in module `{module}` must target a declared job")]
    EventSubscriptionMissingJob { module: String, event: String },
    #[error("event subscription `{event}` in module `{module}` targets unknown job `{job}`")]
    UnknownEventSubscriptionJob {
        module: String,
        event: String,
        job: String,
    },
    #[error(
        "event subscription `{event}` in module `{module}` targets job `{job}` with trigger `{trigger:?}`; domain-event subscriptions must target domain-event jobs"
    )]
    EventSubscriptionTriggerMismatch {
        module: String,
        event: String,
        job: String,
        trigger: JobTriggerKind,
    },
}
