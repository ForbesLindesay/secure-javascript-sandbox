use wasmtime::ResourceLimiter;
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxView, WasiView};
use wasmtime_wasi_http::types::HostFutureIncomingResponse;
use wasmtime_wasi_http::{WasiHttpCtx, WasiHttpView};

use crate::MemoryLimitBytes;
use crate::http::{HttpMode, Requests, send_request_handler};
use crate::memory::{MemoryLimits, TableLimit};

pub(crate) struct SandboxState {
    pub wasi_ctx: WasiCtx,
    pub resource_table: ResourceTable,
    pub limits: MemoryLimits,
    pub wasi_http: WasiHttpCtx,
    pub http: HttpMode,
    pub max_requested_memory_bytes: Option<usize>,
    pub max_requested_table_elements: Option<usize>,
    pub requests: Requests,
}

impl WasiView for SandboxState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi_ctx,
            table: &mut self.resource_table,
        }
    }
}

impl WasiHttpView for SandboxState {
    fn ctx(&mut self) -> &mut WasiHttpCtx {
        &mut self.wasi_http
    }
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.resource_table
    }
    fn send_request(
        &mut self,
        request: hyper::Request<wasmtime_wasi_http::body::HyperOutgoingBody>,
        config: wasmtime_wasi_http::types::OutgoingRequestConfig,
    ) -> wasmtime_wasi_http::HttpResult<wasmtime_wasi_http::types::HostFutureIncomingResponse> {
        let http_mode = self.http.clone();
        let requests = self.requests.clone();
        let handle = wasmtime_wasi::runtime::spawn(async move {
            let result = send_request_handler(request, config, &http_mode, requests).await;
            Ok(result)
        });
        Ok(HostFutureIncomingResponse::pending(handle))
    }
}

impl ResourceLimiter for SandboxState {
    fn memory_growing(
        &mut self,
        _current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> anyhow::Result<bool> {
        self.max_requested_memory_bytes = match self.max_requested_memory_bytes {
            Some(current_max) if desired < current_max => Some(current_max),
            _ => Some(desired),
        };
        let allow = match self.limits.memory_size_bytes {
            MemoryLimitBytes::Limited(limit) if desired > limit => false,
            _ => match maximum {
                Some(max) if desired > max => false,
                _ => true,
            },
        };
        if !allow && self.limits.trap_on_grow_failure {
            anyhow::bail!("forcing trap when growing memory to {desired} bytes")
        } else {
            Ok(allow)
        }
    }

    fn memory_grow_failed(&mut self, error: anyhow::Error) -> anyhow::Result<()> {
        if self.limits.trap_on_grow_failure {
            Err(error.context("forcing a memory growth failure to be a trap"))
        } else {
            // log::debug!("ignoring memory growth failure error: {error:?}");
            Ok(())
        }
    }

    fn table_growing(
        &mut self,
        _current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> anyhow::Result<bool> {
        self.max_requested_table_elements = match self.max_requested_table_elements {
            Some(current_max) if desired < current_max => Some(current_max),
            _ => Some(desired),
        };
        let allow = match self.limits.table_elements {
            TableLimit::Limited(limit) if desired > limit => false,
            _ => match maximum {
                Some(max) if desired > max => false,
                _ => true,
            },
        };
        if !allow && self.limits.trap_on_grow_failure {
            anyhow::bail!("forcing trap when growing table to {desired} elements")
        } else {
            Ok(allow)
        }
    }

    fn table_grow_failed(&mut self, error: anyhow::Error) -> anyhow::Result<()> {
        if self.limits.trap_on_grow_failure {
            Err(error.context("forcing a table growth failure to be a trap"))
        } else {
            // log::debug!("ignoring table growth failure error: {error:?}");
            Ok(())
        }
    }

    fn instances(&self) -> usize {
        self.limits.instances.into()
    }

    fn tables(&self) -> usize {
        self.limits.tables.into()
    }

    fn memories(&self) -> usize {
        self.limits.memories.into()
    }
}
