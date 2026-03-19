#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LocalCacheBackend {
    Moka,
}

impl std::fmt::Display for LocalCacheBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Moka => f.write_str("moka"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DistributedCacheBackend {
    Redis,
    Valkey,
}

impl std::fmt::Display for DistributedCacheBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Redis => f.write_str("redis"),
            Self::Valkey => f.write_str("valkey"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RequestCoalescingMode {
    Disabled,
    Local,
    Cluster,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheTopology {
    l1: LocalCacheBackend,
    l2: Option<DistributedCacheBackend>,
    request_coalescing: RequestCoalescingMode,
}

impl CacheTopology {
    pub fn moka_only() -> Self {
        Self {
            l1: LocalCacheBackend::Moka,
            l2: None,
            request_coalescing: RequestCoalescingMode::Local,
        }
    }

    pub fn with_redis() -> Self {
        Self {
            l1: LocalCacheBackend::Moka,
            l2: Some(DistributedCacheBackend::Redis),
            request_coalescing: RequestCoalescingMode::Cluster,
        }
    }

    pub fn with_valkey() -> Self {
        Self {
            l1: LocalCacheBackend::Moka,
            l2: Some(DistributedCacheBackend::Valkey),
            request_coalescing: RequestCoalescingMode::Cluster,
        }
    }

    pub fn l1(&self) -> LocalCacheBackend {
        self.l1
    }

    pub fn l2(&self) -> Option<DistributedCacheBackend> {
        self.l2
    }

    pub fn request_coalescing_mode(&self) -> RequestCoalescingMode {
        self.request_coalescing
    }

    pub fn supports_shared_invalidation(&self) -> bool {
        self.l2.is_some()
    }

    pub fn supports_shared_coalescing(&self) -> bool {
        matches!(self.request_coalescing, RequestCoalescingMode::Cluster)
    }
}
