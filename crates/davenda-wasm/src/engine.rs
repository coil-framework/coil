use std::time::Instant;

use wasmtime::{Caller, Config, Engine, Linker, Module, Store, StoreLimits, StoreLimitsBuilder};

use crate::error::WasmModelError;
use crate::grants::HostCapabilityGrant;
use crate::invocation::{ExecutionReceipt, HostCall, InvocationOutcome, WasmExecutionSession};
use crate::output::TypedExecutionOutput;

#[derive(Debug, Clone)]
pub struct WasmEngine {
    engine: Engine,
}

impl Default for WasmEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl WasmEngine {
    pub fn new() -> Self {
        let mut config = Config::new();
        config.consume_fuel(true);
        let engine = Engine::new(&config).expect("static wasmtime configuration must be valid");
        Self { engine }
    }

    pub fn compile_module(&self, bytes: &[u8]) -> Result<CompiledWasmModule, WasmModelError> {
        let module =
            Module::new(&self.engine, bytes).map_err(|error| WasmModelError::EngineCompile {
                reason: error.to_string(),
            })?;
        Ok(CompiledWasmModule { module })
    }

    pub fn execute_session(
        &self,
        module: &CompiledWasmModule,
        session: WasmExecutionSession,
        export: &str,
    ) -> Result<ExecutionReceipt, WasmModelError> {
        module.execute(&self.engine, session, export)
    }
}

#[derive(Debug, Clone)]
pub struct CompiledWasmModule {
    module: Module,
}

#[derive(Debug)]
struct EngineHostState {
    session: WasmExecutionSession,
    grants: Vec<HostCapabilityGrant>,
    last_error: Option<WasmModelError>,
    limits: StoreLimits,
}

impl EngineHostState {
    fn new(session: WasmExecutionSession) -> Self {
        let limits = StoreLimitsBuilder::new()
            .memory_size(session.plan().limits.max_memory_bytes as usize)
            .instances(1)
            .tables(4)
            .memories(1)
            .build();

        Self {
            grants: session.grant_slots(),
            session,
            last_error: None,
            limits,
        }
    }

    fn record_slot_call(&mut self, slot: i32, metric: i64) -> Result<i32, WasmModelError> {
        if slot < 0 {
            return Err(WasmModelError::InvalidHostCapabilitySlot {
                handler_id: self.session.plan().handler_id.to_string(),
                slot,
            });
        }

        let grant = self.grants.get(slot as usize).ok_or_else(|| {
            WasmModelError::InvalidHostCapabilitySlot {
                handler_id: self.session.plan().handler_id.to_string(),
                slot,
            }
        })?;
        let call = host_call_for_grant(&self.session, grant, metric)?;
        self.session.record_host_call(call)?;
        Ok(0)
    }
}

impl CompiledWasmModule {
    pub fn execute(
        &self,
        engine: &Engine,
        session: WasmExecutionSession,
        export: &str,
    ) -> Result<ExecutionReceipt, WasmModelError> {
        let export = crate::validation::validate_token("export", export.to_string())?;
        let mut store = Store::new(engine, EngineHostState::new(session));
        store.limiter(|state| &mut state.limits);

        let fuel_budget = store
            .data()
            .session
            .plan()
            .limits
            .max_runtime
            .as_millis()
            .max(1) as u64
            * 10_000;
        store
            .set_fuel(fuel_budget)
            .map_err(|error| WasmModelError::EngineInstantiate {
                handler_id: store.data().session.plan().handler_id.to_string(),
                reason: error.to_string(),
            })?;

        let mut linker = Linker::new(engine);
        linker
            .func_wrap(
                "davenda",
                "host_call",
                |mut caller: Caller<'_, EngineHostState>,
                 slot: i32,
                 metric: i64|
                 -> Result<i32, wasmtime::Error> {
                    let state = caller.data_mut();
                    match state.record_slot_call(slot, metric) {
                        Ok(result) => Ok(result),
                        Err(error) => {
                            state.last_error = Some(error);
                            Err(wasmtime::Error::msg("davenda host call failed"))
                        }
                    }
                },
            )
            .map_err(|error| WasmModelError::EngineInstantiate {
                handler_id: store.data().session.plan().handler_id.to_string(),
                reason: error.to_string(),
            })?;

        let start = Instant::now();
        let instance = linker
            .instantiate(&mut store, &self.module)
            .map_err(|error| WasmModelError::EngineInstantiate {
                handler_id: store.data().session.plan().handler_id.to_string(),
                reason: error.to_string(),
            })?;
        let handler_id = store.data().session.plan().handler_id.to_string();
        let function = instance
            .get_typed_func::<(), i32>(&mut store, &export)
            .map_err(|_| WasmModelError::EngineExportMissing {
                handler_id: handler_id.clone(),
                export: export.clone(),
            })?;
        let outcome_code = function.call(&mut store, ()).map_err(|error| {
            if let Some(host_error) = store.data().last_error.clone() {
                host_error
            } else {
                WasmModelError::EngineTrap {
                    handler_id: handler_id.clone(),
                    reason: error.to_string(),
                }
            }
        })?;
        let runtime = start.elapsed();
        let typed_output = read_typed_output(&mut store, &instance, &handler_id)?;

        let state = store.into_data();
        if let Some(host_error) = state.last_error {
            return Err(host_error);
        }

        let outcome = InvocationOutcome::from_engine_code(
            outcome_code,
            state.session.plan().handler_id.to_string(),
        )?;
        state.session.finish(runtime, outcome, typed_output)
    }
}

fn read_typed_output(
    store: &mut Store<EngineHostState>,
    instance: &wasmtime::Instance,
    handler_id: &str,
) -> Result<Option<TypedExecutionOutput>, WasmModelError> {
    let Some(export) = instance.get_func(&mut *store, TypedExecutionOutput::ABI_EXPORT) else {
        return Ok(None);
    };
    let func = export.typed::<(), i64>(&mut *store).map_err(|error| {
        WasmModelError::EngineInstantiate {
            handler_id: handler_id.to_string(),
            reason: format!(
                "typed return export `{}` has unexpected signature: {error}",
                TypedExecutionOutput::ABI_EXPORT
            ),
        }
    })?;
    let packed = func
        .call(&mut *store, ())
        .map_err(|error| WasmModelError::EngineTrap {
            handler_id: handler_id.to_string(),
            reason: error.to_string(),
        })?;
    let packed = packed as u64;
    let ptr = (packed & 0xffff_ffff) as u32 as usize;
    let len = (packed >> 32) as u32 as usize;

    let memory = instance.get_memory(&mut *store, "memory").ok_or_else(|| {
        WasmModelError::InvalidTypedReturn {
            reason: format!(
                "typed return export `{}` is present but no `memory` export exists",
                TypedExecutionOutput::ABI_EXPORT
            ),
        }
    })?;

    let mut bytes = vec![0u8; len];
    memory.read(&mut *store, ptr, &mut bytes).map_err(|error| {
        WasmModelError::InvalidTypedReturn {
            reason: format!("failed to read typed return payload: {error}"),
        }
    })?;

    TypedExecutionOutput::decode(&bytes).map(Some)
}

fn host_call_for_grant(
    session: &WasmExecutionSession,
    grant: &HostCapabilityGrant,
    metric: i64,
) -> Result<HostCall, WasmModelError> {
    let metric = u64::try_from(metric).map_err(|_| WasmModelError::InvalidHostCallMetric {
        handler_id: session.plan().handler_id.to_string(),
        metric,
    })?;

    Ok(match grant {
        HostCapabilityGrant::DataRead { resource } => HostCall::DataRead {
            resource: resource.clone(),
        },
        HostCapabilityGrant::DataWrite { resource } => HostCall::DataWrite {
            resource: resource.clone(),
        },
        HostCapabilityGrant::AuthCheck => HostCall::AuthCheck,
        HostCapabilityGrant::AuthList => HostCall::AuthList,
        HostCapabilityGrant::AuthLookup => HostCall::AuthLookup,
        HostCapabilityGrant::AuthTupleWrite => HostCall::AuthTupleWrite,
        HostCapabilityGrant::StorageRead { class } => HostCall::StorageRead { class: *class },
        HostCapabilityGrant::StorageWrite { class } => HostCall::StorageWrite {
            class: *class,
            bytes: metric,
        },
        HostCapabilityGrant::RenderFragment { slot } => {
            HostCall::RenderFragment { slot: slot.clone() }
        }
        HostCapabilityGrant::MetadataWrite { kind } => HostCall::MetadataWrite { kind: *kind },
        HostCapabilityGrant::CacheHintWrite => HostCall::CacheHintWrite,
        HostCapabilityGrant::OutboundHttp { integration } => HostCall::OutboundHttp {
            integration: integration.clone(),
            response_bytes: metric,
        },
        HostCapabilityGrant::SecretRead { secret } => HostCall::SecretRead {
            secret: secret.clone(),
        },
        HostCapabilityGrant::EnqueueJob { queue } => HostCall::EnqueueJob {
            queue: queue.clone(),
        },
    })
}
