/// Submit blocking outbound HTTP work to Tokio's blocking pool.
///
/// The request thread only waits at the boundary; the actual network call and
/// body read run off the core worker lane when a multi-thread runtime is
/// present.
pub(super) fn offload_outbound_http_to_blocking_pool<T, F>(operation: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String> + Send + 'static,
    T: Send + 'static,
{
    if matches!(
        tokio::runtime::Handle::try_current()
            .ok()
            .map(|handle| handle.runtime_flavor()),
        Some(tokio::runtime::RuntimeFlavor::MultiThread)
    ) {
        let handle = tokio::runtime::Handle::current();
        let join = handle.spawn_blocking(operation);
        tokio::task::block_in_place(|| {
            handle.block_on(async move {
                join.await.map_err(|error| {
                    format!("failed to execute outbound HTTP on the blocking pool: {error}")
                })?
            })
        })
    } else {
        operation()
    }
}
