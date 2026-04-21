use slog::{Logger, debug, error, info, warn};
use sqlx::{Pool, Postgres};
use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::io::AsyncWrite;
use wasmtime::component::{Component, Linker, ResourceTable, Val};
use wasmtime::*;
use wasmtime_wasi::cli::{IsTerminal, StdoutStream};
use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView};
use wasmtime_wasi_http::WasiHttpCtx;
use wasmtime_wasi_http::p2::{
    HttpResult, WasiHttpCtxView, WasiHttpHooks, WasiHttpView, body::HyperOutgoingBody,
    default_send_request, types::HostFutureIncomingResponse, types::OutgoingRequestConfig,
};

use crate::api::task::{Task, Val as ProtoVal, val};
use crate::orm;

/// A line-buffered [`AsyncWrite`] that forwards each complete line to `handler`.
struct LineWriter {
    handler: Arc<dyn Fn(String) + Send + Sync>,
    buffer: Vec<u8>,
}

impl LineWriter {
    fn flush_lines(&mut self, include_incomplete: bool) {
        let mut start = 0;
        loop {
            match self.buffer[start..].iter().position(|&b| b == b'\n') {
                Some(pos) => {
                    let line =
                        String::from_utf8_lossy(&self.buffer[start..start + pos]).into_owned();
                    (self.handler)(line);
                    start += pos + 1;
                }
                None => break,
            }
        }
        if include_incomplete && start < self.buffer.len() {
            let line = String::from_utf8_lossy(&self.buffer[start..]).into_owned();
            (self.handler)(line);
            start = self.buffer.len();
        }
        if start > 0 {
            self.buffer.drain(..start);
        }
    }
}

impl AsyncWrite for LineWriter {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.buffer.extend_from_slice(buf);
        self.flush_lines(false);
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.flush_lines(true);
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.flush_lines(true);
        Poll::Ready(Ok(()))
    }
}

/// A [`StdoutStream`] that calls `handler` for every line written by the guest.
///
/// Both stdout and stderr can be wired up independently:
/// ```rust
/// .stdout(LogStream::new(move |line| info!(logger, "{}", line)))
/// .stderr(LogStream::new(move |line| error!(logger, "{}", line)))
/// ```
pub struct LogStream {
    handler: Arc<dyn Fn(String) + Send + Sync>,
}

impl LogStream {
    pub fn new(handler: impl Fn(String) + Send + Sync + 'static) -> Self {
        Self {
            handler: Arc::new(handler),
        }
    }
}

impl IsTerminal for LogStream {
    fn is_terminal(&self) -> bool {
        false
    }
}

impl StdoutStream for LogStream {
    fn async_stream(&self) -> Box<dyn AsyncWrite + Send + Sync> {
        Box::new(LineWriter {
            handler: Arc::clone(&self.handler),
            buffer: Vec::new(),
        })
    }
}

pub struct ComponentRunStates {
    // These two are required basically as a standard way to enable the impl of IoView and
    // WasiView.
    // impl of WasiView is required by [`wasmtime_wasi::p2::add_to_linker_sync`]
    pub wasi_ctx: WasiCtx,
    pub resource_table: ResourceTable,
    pub http_ctx: WasiHttpCtx,
    pub http_hooks: LoggingHttpHooks,
}

impl WasiView for ComponentRunStates {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi_ctx,
            table: &mut self.resource_table,
        }
    }
}

pub struct LoggingHttpHooks {
    logger: Logger,
}

impl WasiHttpHooks for LoggingHttpHooks {
    fn send_request(
        &mut self,
        request: hyper::Request<HyperOutgoingBody>,
        config: OutgoingRequestConfig,
    ) -> HttpResult<HostFutureIncomingResponse> {
        // Blocking of requests is possible here
        let uri = request.uri().to_string();
        debug!(self.logger, "Processing http request"; "uri" => uri);
        Ok(default_send_request(request, config))
    }
}

impl WasiHttpView for ComponentRunStates {
    fn http(&mut self) -> WasiHttpCtxView<'_> {
        WasiHttpCtxView {
            ctx: &mut self.http_ctx,
            table: &mut self.resource_table,
            hooks: &mut self.http_hooks,
        }
    }
}

/// Shared Wasm runtime that holds the pre-configured [`Engine`] and [`Linker`].
///
/// Create once at application startup and share (via `Arc<WasmHost>`) across
/// task executions. Per-execution isolation is provided by a fresh [`Store`].
pub struct WasmHost {
    engine: Engine,
    linker: Linker<ComponentRunStates>,
}

impl WasmHost {
    pub fn new() -> Result<Self> {
        // Speed up instantiation using PoolingAllocation: https://docs.wasmtime.dev/examples-fast-instantiation.html#tuning-wasmtime-for-fast-instantiation
        let mut pool = PoolingAllocationConfig::new();
        let max_memory = 1 << 32; // 4 GiB or 32bit memory space
        pool.max_memory_size(max_memory);

        // Speed up compilation using Cache: https://docs.wasmtime.dev/examples-fast-compilation.html#tuning-wasmtime-for-fast-compilation
        let cache = match Cache::new(CacheConfig::new()) {
            Ok(value) => value,
            Err(err) => return Err(err),
        };

        let mut config = Config::new();
        config.allocation_strategy(InstanceAllocationStrategy::Pooling(pool));
        config.memory_init_cow(true);
        config.wasm_memory64(true);
        config.memory_reservation(max_memory as u64);
        config.cache(Some(cache));
        config.parallel_compilation(true);

        let engine = Engine::new(&config)?;

        let mut linker = Linker::new(&engine);
        wasmtime_wasi::p2::add_to_linker_async(&mut linker)?;
        wasmtime_wasi_http::p2::add_only_http_to_linker_async(&mut linker)?;

        Ok(Self { engine, linker })
    }

    pub async fn execute(
        &self,
        logger: Logger,
        db: Pool<Postgres>,
        task: Task,
        task_id: String,
        artifact: Vec<u8>,
    ) -> Result<Vec<u8>> {
        let component = Component::from_binary(&self.engine, &artifact)?;
        debug!(logger, "Loaded component");

        let socker_addr_check_logger = logger.new(slog::o!());
        let stdout_logger = logger.new(slog::o!());
        let stderr_logger = logger.new(slog::o!());
        let stdout_db = db.clone();
        let stderr_db = db.clone();
        let task_id_stdout = task_id.clone();
        let task_id_stderr = task_id.clone();
        let wasi = WasiCtx::builder()
            .stdout(LogStream::new(move |line| {
                info!(stdout_logger, "Artifact: {}", line);
                let task_id = task_id_stdout.clone();
                tokio::spawn(orm::log(
                    stdout_db.clone(),
                    task_id,
                    orm::LogLevel::Error,
                    orm::LogIssuer::Artifact,
                    line,
                ));
            }))
            .stderr(LogStream::new(move |line| {
                error!(stderr_logger, "Artifact: {}", line);
                let task_id = task_id_stderr.clone();
                tokio::spawn(orm::log(
                    stderr_db.clone(),
                    task_id,
                    orm::LogLevel::Error,
                    orm::LogIssuer::Artifact,
                    line,
                ));
            }))
            .socket_addr_check(move |_address, _reason| {
                // Blocking of socket requests is possible here
                warn!(socker_addr_check_logger, "Rejected socked request");
                Box::pin(async move { false })
            })
            .args(&task.arguments)
            .envs(
                &task
                    .environment_variables
                    .into_iter()
                    .map(|e| (e.key, e.value))
                    .collect::<Vec<_>>(),
            )
            .build();
        debug!(logger, "Built WASI context");

        let state = ComponentRunStates {
            wasi_ctx: wasi,
            resource_table: ResourceTable::new(),
            http_ctx: WasiHttpCtx::new(),
            http_hooks: LoggingHttpHooks {
                logger: logger.new(slog::o!()),
            },
        };

        let mut store = Store::new(&self.engine, state);
        let instance = self
            .linker
            .instantiate_async(&mut store, &component)
            .await?;
        debug!(logger, "Initialized component");

        let function = task
            .function
            .as_ref()
            .ok_or_else(|| Error::msg("Task has no function"))?;
        let artifact = function
            .artifact
            .as_ref()
            .ok_or_else(|| Error::msg("Function has no artifact"))?;
        let package = artifact
            .package
            .as_ref()
            .ok_or_else(|| Error::msg("Artifact has no package"))?;

        let interface_identifier = format!(
            "{}:{}/{}",
            package.namespace, package.name, function.interface,
        );

        // Get the index for the exported interface
        let interface_idx = instance
            .get_export_index(&mut store, None, &interface_identifier)
            .ok_or_else(|| Error::msg(format!("Cannot get interface {}", interface_identifier)))?;
        // Get the index for the exported function in the exported interface
        let parent_export_idx = Some(&interface_idx);
        let func_idx = instance
            .get_export_index(&mut store, parent_export_idx, &function.name)
            .ok_or_else(|| {
                Error::msg(format!(
                    "Cannot get function {} in interface {}",
                    function.name, interface_identifier,
                ))
            })?;

        let func = instance
            .get_func(&mut store, func_idx)
            .expect("Unreachable since we've got func_idx");
        debug!(logger, "Found function");

        let mut return_values = vec![Val::String("".to_string()); 1];
        let params = convert_to_wasm_params(task.parameters)?;
        func.call_async(&mut store, params.as_slice(), &mut return_values)
            .await?;

        debug!(logger, "Finished execution");

        // Expect the function to return a tuple (result_string, error_string)
        match &return_values[0] {
            Val::Tuple(items) if items.len() == 2 => match (&items[0], &items[1]) {
                (Val::String(result), Val::String(err)) => {
                    if err.is_empty() {
                        info!(logger, "Execution result: {}", result);
                        Ok(result.as_bytes().to_vec())
                    } else {
                        Err(Error::msg(format!("Execution error: {}", err)))
                    }
                }
                _ => Err(Error::msg(
                    "Return value tuple does not contain two strings",
                )),
            },
            _ => Err(Error::msg("Return value is not a two-element tuple")),
        }
    }
}

fn convert_to_wasm_params(task_params: Vec<ProtoVal>) -> Result<Vec<Val>> {
    task_params.into_iter().map(proto_val_to_wasm_val).collect()
}

fn proto_val_to_wasm_val(proto: ProtoVal) -> Result<Val> {
    use val::Value;
    match proto
        .value
        .ok_or_else(|| Error::msg("Val has no value set"))?
    {
        Value::BoolVal(v) => Ok(Val::Bool(v)),
        Value::S8Val(v) => Ok(Val::S8(v as i8)),
        Value::U8Val(v) => Ok(Val::U8(v as u8)),
        Value::S16Val(v) => Ok(Val::S16(v as i16)),
        Value::U16Val(v) => Ok(Val::U16(v as u16)),
        Value::S32Val(v) => Ok(Val::S32(v)),
        Value::U32Val(v) => Ok(Val::U32(v)),
        Value::S64Val(v) => Ok(Val::S64(v)),
        Value::U64Val(v) => Ok(Val::U64(v)),
        Value::F32Val(v) => Ok(Val::Float32(v)),
        Value::F64Val(v) => Ok(Val::Float64(v)),
        Value::CharVal(v) => {
            let c = char::from_u32(v)
                .ok_or_else(|| Error::msg(format!("Invalid Unicode scalar value: {v}")))?;
            Ok(Val::Char(c))
        }
        Value::StringVal(v) => Ok(Val::String(v)),
        Value::ListVal(list) => {
            let items = list
                .values
                .into_iter()
                .map(proto_val_to_wasm_val)
                .collect::<Result<Vec<_>>>()?;
            Ok(Val::List(items))
        }
        Value::TupleVal(tuple) => {
            let items = tuple
                .values
                .into_iter()
                .map(proto_val_to_wasm_val)
                .collect::<Result<Vec<_>>>()?;
            Ok(Val::Tuple(items))
        }
        Value::OptionVal(opt) => {
            // opt.value: Option<Box<Val>> (boxed to break the recursive size cycle)
            let inner = opt
                .value
                .map(|v| proto_val_to_wasm_val(*v))
                .transpose()?
                .map(Box::new);
            Ok(Val::Option(inner))
        }
        Value::ResultVal(res) => {
            let inner = res
                .value
                .map(|v| proto_val_to_wasm_val(*v))
                .transpose()?
                .map(Box::new);
            Ok(Val::Result(if res.is_ok { Ok(inner) } else { Err(inner) }))
        }
        Value::RecordVal(record) => {
            let fields = record
                .fields
                .into_iter()
                .map(|f| {
                    let v = f.value.ok_or_else(|| {
                        Error::msg(format!("Record field '{}' has no value", f.name))
                    })?;
                    Ok((f.name, proto_val_to_wasm_val(*v)?))
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(Val::Record(fields))
        }
        Value::VariantVal(variant) => {
            let inner = variant
                .value
                .map(|v| proto_val_to_wasm_val(*v))
                .transpose()?
                .map(Box::new);
            Ok(Val::Variant(variant.name, inner))
        }
        Value::EnumVal(name) => Ok(Val::Enum(name)),
        Value::FlagsVal(flags) => Ok(Val::Flags(flags.flags)),
    }
}
