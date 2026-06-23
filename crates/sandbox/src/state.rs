use wasmtime::ResourceLimiter;
use wasmtime::component::HasData;
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxView, WasiView};
use wasmtime_wasi_http::p2::bindings::http::types::ErrorCode;
use wasmtime_wasi_http::p2::types::HostFutureIncomingResponse;
use wasmtime_wasi_http::{
    WasiHttpCtx,
    p2::{WasiHttpCtxView, WasiHttpHooks, WasiHttpView},
};

use crate::http::{OutboundRequest, send_request_handler};
use crate::memory::MemoryLimits;
use crate::shared_vec::SharedVec;
use crate::{
    CustomHttpMode, CustomImportMap, RequestLimit, RequestValidationOutcome, ResolvedModule,
};

pub(crate) struct SandboxState<TImportMap: CustomImportMap, THttpMode: CustomHttpMode> {
    pub wasi_ctx: WasiCtx,
    pub resource_table: ResourceTable,
    pub memory_limits: MemoryLimits,
    pub wasi_http: WasiHttpCtx,
    pub http: SandboxHttpState<THttpMode>,
    pub imports: TImportMap,
    pub max_requested_memory_bytes: Option<usize>,
    pub max_requested_table_elements: Option<usize>,
}

impl<TImportMap: CustomImportMap, THttpMode: CustomHttpMode> WasiView
    for SandboxState<TImportMap, THttpMode>
{
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi_ctx,
            table: &mut self.resource_table,
        }
    }
}

impl<TImportMap: CustomImportMap, THttpMode: CustomHttpMode> WasiHttpView
    for SandboxState<TImportMap, THttpMode>
{
    fn http(&mut self) -> WasiHttpCtxView<'_> {
        WasiHttpCtxView {
            ctx: &mut self.wasi_http,
            table: &mut self.resource_table,
            hooks: &mut self.http,
        }
    }
}
pub(crate) struct SandboxHttpState<THttpMode: CustomHttpMode> {
    pub request_count: usize,
    pub requests: SharedVec<OutboundRequest>,
    pub request_limit: RequestLimit,
    pub http: THttpMode,
}
impl<THttpMode: CustomHttpMode> WasiHttpHooks for SandboxHttpState<THttpMode> {
    fn send_request(
        &mut self,
        request: hyper::Request<wasmtime_wasi_http::p2::body::HyperOutgoingBody>,
        config: wasmtime_wasi_http::p2::types::OutgoingRequestConfig,
    ) -> wasmtime_wasi_http::p2::HttpResult<wasmtime_wasi_http::p2::types::HostFutureIncomingResponse>
    {
        self.request_count = self.request_count.saturating_add(1);
        if !self.request_limit.is_within_bound(self.request_count) {
            self.requests.push(OutboundRequest(
                request.uri().clone(),
                None,
                RequestValidationOutcome::Blocked,
            ));
            return Ok(HostFutureIncomingResponse::ready(Ok(Err(
                ErrorCode::ConnectionLimitReached,
            ))));
        }
        let http_mode = self.http.clone();
        let requests = self.requests.clone();
        let handle = wasmtime_wasi::runtime::spawn(async move {
            let result = send_request_handler(request, config, &http_mode, requests).await;
            Ok(result)
        });
        Ok(HostFutureIncomingResponse::pending(handle))
    }
}

impl<TImportMap: CustomImportMap, THttpMode: CustomHttpMode> ResourceLimiter
    for SandboxState<TImportMap, THttpMode>
{
    fn memory_growing(
        &mut self,
        _current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        self.max_requested_memory_bytes = match self.max_requested_memory_bytes {
            Some(current_max) if desired < current_max => Some(current_max),
            _ => Some(desired),
        };
        let allow = self
            .memory_limits
            .memory_size_bytes
            .is_within_bound(desired)
            && maximum.is_none_or(|max| desired <= max);
        if !allow && self.memory_limits.trap_on_grow_failure {
            Err(wasmtime::format_err!(
                "forcing trap when growing memory to {desired} bytes"
            ))
        } else {
            Ok(allow)
        }
    }

    fn memory_grow_failed(&mut self, error: wasmtime::Error) -> wasmtime::Result<()> {
        if self.memory_limits.trap_on_grow_failure {
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
    ) -> wasmtime::Result<bool> {
        self.max_requested_table_elements = match self.max_requested_table_elements {
            Some(current_max) if desired < current_max => Some(current_max),
            _ => Some(desired),
        };
        let allow = self.memory_limits.table_elements.is_within_bound(desired)
            && maximum.is_none_or(|max| desired <= max);
        if !allow && self.memory_limits.trap_on_grow_failure {
            Err(wasmtime::format_err!(
                "forcing trap when growing table to {desired} elements"
            ))
        } else {
            Ok(allow)
        }
    }

    fn table_grow_failed(&mut self, error: wasmtime::Error) -> wasmtime::Result<()> {
        if self.memory_limits.trap_on_grow_failure {
            Err(error.context("forcing a table growth failure to be a trap"))
        } else {
            // log::debug!("ignoring table growth failure error: {error:?}");
            Ok(())
        }
    }

    fn instances(&self) -> usize {
        self.memory_limits.instances.into()
    }

    fn tables(&self) -> usize {
        self.memory_limits.tables.into()
    }

    fn memories(&self) -> usize {
        self.memory_limits.memories.into()
    }
}

impl<TImportMap: CustomImportMap, THttpMode: CustomHttpMode> HasData
    for SandboxState<TImportMap, THttpMode>
{
    type Data<'a> = &'a mut Self;
}
impl<TImportMap: CustomImportMap, THttpMode: CustomHttpMode> crate::sandbox::Host
    for SandboxState<TImportMap, THttpMode>
{
    async fn resolve_import_path(
        &mut self,
        path: String,
        parent: String,
    ) -> Result<crate::sandbox::ResolvedModule, String> {
        let resolved = self
            .imports
            .resolve_import_path(path, parent)
            .map_err(|e| e.to_string())?;
        Ok(match resolved {
            ResolvedModule::Url(url) => crate::sandbox::ResolvedModule::Url(url),
            ResolvedModule::Id(id) => crate::sandbox::ResolvedModule::Id(id),
        })
    }
    async fn load_import(&mut self, id: String) -> Result<String, String> {
        self.imports.load_import(id).map_err(|e| e.to_string())
    }
}

// {
//     let request = hyper::Request::builder()
//         .method(hyper::Method::GET)
//         .uri(&id)
//         .body(String::new())
//         .map_err(|e| anyhow::anyhow!("Failed to build request for {}: {}", id, e))?;
//     let result = crate::http::send_request_handler(request, Default::default(), http_mode);
// }
