use std::error::Error;
use std::fmt;

use crate::grants::HostCapabilityGrant;
use crate::host_api::HostServiceDomain;
use crate::ids::{ContractVersion, ExtensionPointKind, HttpMethod};
use crate::invocation::PrincipalKind;
use crate::manifest::ExtensionConfigValueType;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WasmModelError {
    EmptyField {
        field: &'static str,
    },
    InvalidToken {
        field: &'static str,
        value: String,
    },
    InvalidChecksum {
        field: &'static str,
        value: String,
    },
    ArtifactRead {
        path: String,
        reason: String,
    },
    ArtifactChecksumMismatch {
        path: String,
        expected: String,
        actual: String,
    },
    DuplicateConfigField {
        key: String,
    },
    UnknownConfigField {
        key: String,
    },
    MissingRequiredConfigField {
        key: String,
    },
    ConfigTypeMismatch {
        key: String,
        expected: ExtensionConfigValueType,
        actual: ExtensionConfigValueType,
    },
    InvalidConfigValue {
        key: String,
        reason: String,
    },
    DuplicateJsonLdProperty {
        property: String,
    },
    InvalidJsonLdProperty {
        property: String,
    },
    InvalidJsonLdNumber {
        property: String,
        value: String,
    },
    InvalidRoute {
        field: &'static str,
        route: String,
    },
    DuplicateHandlerId {
        handler_id: String,
    },
    UnsupportedPageMethod {
        method: HttpMethod,
    },
    UnsupportedGrantForPoint {
        handler_id: String,
        point: ExtensionPointKind,
        grant: HostCapabilityGrant,
    },
    HandlerNotFound {
        handler_id: String,
    },
    DuplicateInstalledHandler {
        handler_id: String,
    },
    DuplicateInstalledExtension {
        extension_id: String,
    },
    MixedCustomerAppInstallation {
        extension_id: String,
        expected: String,
        actual: String,
    },
    GrantNotDeclared {
        handler_id: String,
        grant: HostCapabilityGrant,
    },
    HostApiVersionMismatch {
        extension_id: String,
        expected: ContractVersion,
        actual: ContractVersion,
    },
    DuplicateExtensionTarget {
        point: ExtensionPointKind,
        target: String,
        existing_handler: String,
        conflicting_handler: String,
    },
    LimitOverrideExceedsDeclared {
        handler_id: String,
        field: &'static str,
    },
    ZeroLimit {
        field: &'static str,
    },
    PrincipalIdRequired {
        kind: PrincipalKind,
    },
    InvocationPointMismatch {
        handler_id: String,
        expected: ExtensionPointKind,
        actual: ExtensionPointKind,
    },
    InvocationTargetMismatch {
        handler_id: String,
        detail: String,
    },
    UnverifiedWebhook {
        handler_id: String,
    },
    ReplayUnsafeWebhook {
        handler_id: String,
    },
    HostGrantDenied {
        handler_id: String,
        grant: HostCapabilityGrant,
    },
    HostServiceUnavailable {
        handler_id: String,
        domain: HostServiceDomain,
        reason: String,
    },
    ResourceLimitExceeded {
        handler_id: String,
        field: &'static str,
    },
    ZeroSchemaVersion {
        field: &'static str,
    },
    InvalidOutcomeForPoint {
        handler_id: String,
        point: ExtensionPointKind,
        outcome: &'static str,
    },
    RuntimeBudgetExceeded {
        handler_id: String,
        max_runtime: std::time::Duration,
        actual_runtime: std::time::Duration,
    },
    EngineCompile {
        reason: String,
    },
    EngineInstantiate {
        handler_id: String,
        reason: String,
    },
    EngineExportMissing {
        handler_id: String,
        export: String,
    },
    EngineTrap {
        handler_id: String,
        reason: String,
    },
    InvalidHostCapabilitySlot {
        handler_id: String,
        slot: i32,
    },
    InvalidHostCallMetric {
        handler_id: String,
        metric: i64,
    },
    InvalidOutcomeCode {
        handler_id: String,
        code: i32,
    },
    InvalidTypedStatus {
        status: u16,
    },
    InvalidTypedReturn {
        reason: String,
    },
    TypedReturnPointMismatch {
        expected: ExtensionPointKind,
        actual: ExtensionPointKind,
    },
    TypedReturnBodyMismatch {
        point: ExtensionPointKind,
        body: String,
    },
}

impl fmt::Display for WasmModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(f, "`{field}` cannot be empty"),
            Self::InvalidToken { field, value } => {
                write!(f, "`{field}` contains an invalid token `{value}`")
            }
            Self::InvalidChecksum { field, value } => write!(
                f,
                "`{field}` must be a 64-character lowercase hex digest, got `{value}`"
            ),
            Self::ArtifactRead { path, reason } => {
                write!(f, "failed to read artifact at `{path}`: {reason}")
            }
            Self::ArtifactChecksumMismatch {
                path,
                expected,
                actual,
            } => write!(
                f,
                "artifact at `{path}` failed checksum verification: expected `{expected}`, got `{actual}`"
            ),
            Self::DuplicateConfigField { key } => {
                write!(f, "extension config schema declares duplicate key `{key}`")
            }
            Self::UnknownConfigField { key } => {
                write!(
                    f,
                    "extension config field `{key}` is not declared in the package schema"
                )
            }
            Self::MissingRequiredConfigField { key } => {
                write!(
                    f,
                    "extension config field `{key}` is required but was not provided"
                )
            }
            Self::ConfigTypeMismatch {
                key,
                expected,
                actual,
            } => write!(
                f,
                "extension config field `{key}` expects `{expected}` but received `{actual}`"
            ),
            Self::InvalidConfigValue { key, reason } => {
                write!(f, "extension config field `{key}` is invalid: {reason}")
            }
            Self::DuplicateJsonLdProperty { property } => {
                write!(f, "JSON-LD property `{property}` is duplicated")
            }
            Self::InvalidJsonLdProperty { property } => {
                write!(f, "JSON-LD property `{property}` is invalid")
            }
            Self::InvalidJsonLdNumber { property, value } => {
                write!(
                    f,
                    "JSON-LD property `{property}` expects a finite number, got `{value}`"
                )
            }
            Self::InvalidRoute { field, route } => {
                write!(f, "`{field}` must start with `/`, got `{route}`")
            }
            Self::DuplicateHandlerId { handler_id } => {
                write!(
                    f,
                    "extension manifest declares duplicate handler `{handler_id}`"
                )
            }
            Self::UnsupportedPageMethod { method } => {
                write!(f, "page handlers do not support `{method}`")
            }
            Self::UnsupportedGrantForPoint {
                handler_id,
                point,
                grant,
            } => write!(
                f,
                "handler `{handler_id}` for `{point}` cannot request host grant `{grant}`"
            ),
            Self::HandlerNotFound { handler_id } => {
                write!(
                    f,
                    "installed handler `{handler_id}` does not exist in the manifest"
                )
            }
            Self::DuplicateInstalledHandler { handler_id } => {
                write!(f, "handler `{handler_id}` is installed more than once")
            }
            Self::DuplicateInstalledExtension { extension_id } => {
                write!(f, "extension `{extension_id}` is installed more than once")
            }
            Self::MixedCustomerAppInstallation {
                extension_id,
                expected,
                actual,
            } => write!(
                f,
                "extension `{extension_id}` targets customer app `{actual}` but the registry is already bound to `{expected}`"
            ),
            Self::GrantNotDeclared { handler_id, grant } => write!(
                f,
                "handler `{handler_id}` was granted `{grant}` without declaring it in the manifest"
            ),
            Self::HostApiVersionMismatch {
                extension_id,
                expected,
                actual,
            } => write!(
                f,
                "extension `{extension_id}` requires host API `{actual}` but runtime provides `{expected}`"
            ),
            Self::DuplicateExtensionTarget {
                point,
                target,
                existing_handler,
                conflicting_handler,
            } => write!(
                f,
                "extension target `{target}` for `{point}` is already claimed by `{existing_handler}` and cannot also register `{conflicting_handler}`"
            ),
            Self::LimitOverrideExceedsDeclared { handler_id, field } => write!(
                f,
                "handler `{handler_id}` has an installation limit override that is looser for `{field}`"
            ),
            Self::ZeroLimit { field } => write!(f, "`{field}` must be greater than zero"),
            Self::PrincipalIdRequired { kind } => {
                write!(f, "principal kind `{kind}` requires a non-empty id")
            }
            Self::InvocationPointMismatch {
                handler_id,
                expected,
                actual,
            } => write!(
                f,
                "handler `{handler_id}` expects invocation point `{expected}` but received `{actual}`"
            ),
            Self::InvocationTargetMismatch { handler_id, detail } => {
                write!(
                    f,
                    "handler `{handler_id}` cannot handle this invocation: {detail}"
                )
            }
            Self::UnverifiedWebhook { handler_id } => write!(
                f,
                "handler `{handler_id}` cannot run until the host verifies the webhook signature"
            ),
            Self::ReplayUnsafeWebhook { handler_id } => write!(
                f,
                "handler `{handler_id}` cannot run until replay protection has been applied"
            ),
            Self::HostGrantDenied { handler_id, grant } => write!(
                f,
                "handler `{handler_id}` attempted host call `{grant}` without a granted capability"
            ),
            Self::HostServiceUnavailable {
                handler_id,
                domain,
                reason,
            } => write!(
                f,
                "handler `{handler_id}` cannot use `{domain:?}` host service: {reason}"
            ),
            Self::ResourceLimitExceeded { handler_id, field } => write!(
                f,
                "handler `{handler_id}` exceeded its `{field}` resource limit"
            ),
            Self::ZeroSchemaVersion { field } => {
                write!(f, "`{field}` must be greater than zero")
            }
            Self::InvalidOutcomeForPoint {
                handler_id,
                point,
                outcome,
            } => write!(
                f,
                "handler `{handler_id}` for `{point}` returned invalid outcome `{outcome}`"
            ),
            Self::RuntimeBudgetExceeded {
                handler_id,
                max_runtime,
                actual_runtime,
            } => write!(
                f,
                "handler `{handler_id}` exceeded runtime budget {:?} with {:?}",
                max_runtime, actual_runtime
            ),
            Self::EngineCompile { reason } => {
                write!(f, "wasm engine could not compile module: {reason}")
            }
            Self::EngineInstantiate { handler_id, reason } => write!(
                f,
                "handler `{handler_id}` could not be instantiated in the wasm engine: {reason}"
            ),
            Self::EngineExportMissing { handler_id, export } => write!(
                f,
                "handler `{handler_id}` expected wasm export `{export}` but it was not present"
            ),
            Self::EngineTrap { handler_id, reason } => write!(
                f,
                "handler `{handler_id}` trapped during wasm execution: {reason}"
            ),
            Self::InvalidHostCapabilitySlot { handler_id, slot } => write!(
                f,
                "handler `{handler_id}` requested undeclared host capability slot `{slot}`"
            ),
            Self::InvalidHostCallMetric { handler_id, metric } => write!(
                f,
                "handler `{handler_id}` supplied invalid host-call metric `{metric}`"
            ),
            Self::InvalidOutcomeCode { handler_id, code } => write!(
                f,
                "handler `{handler_id}` returned unknown wasm outcome code `{code}`"
            ),
            Self::InvalidTypedStatus { status } => {
                write!(f, "typed HTTP status `{status}` is invalid")
            }
            Self::InvalidTypedReturn { reason } => {
                write!(f, "typed return payload is invalid: {reason}")
            }
            Self::TypedReturnPointMismatch { expected, actual } => write!(
                f,
                "typed return payload targets `{actual}` but invocation point is `{expected}`"
            ),
            Self::TypedReturnBodyMismatch { point, body } => write!(
                f,
                "typed return payload body `{body}` is not valid for `{point}` invocations"
            ),
        }
    }
}

impl Error for WasmModelError {}
