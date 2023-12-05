use super::client;
use super::conf;
use super::message;
use super::method;
use super::sclient;
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

/// * Server spawns a worker thread
/// * Worker thread calls an ApplicationWorkerFactory function to
///   generate an ApplicationWorker.
/// * app_worker.absorb_env() is called to pass the worker a Client
///   and allow for other thread data collection routines.
/// * app_worker.worker_start() is called allowing the worker to
///   perform any other startup routines.
/// * Worker waits for inbound method calls.
/// * Inbound method call arrives
/// * app_worker.start_session() is called on CONNECT any stateless request.
/// * Called method is looked up in the app_worker's methods().
/// * method handler function is called to handle the request.
/// * If a DISCONNECT is received OR its a stateless API call,
///   worker.end_session() is called.
/// * Once all requests are complete in the current session,
///   the Worker goes back to sleep to wait for more requests.
/// * Just before the thread ends/joins, app_worker.worker_end() is called.

/// Function that generates ApplicationWorker implementers.
///
/// This type of function may be cloned and passed through the thread
/// boundary, but the ApplicationWorker's it generates are not
/// guaranteed to be thread-Send-able, hence the factory approach.
pub type ApplicationWorkerFactory = fn() -> Box<dyn ApplicationWorker>;

/// Opaque collection of read-only, thread-Send'able data.
pub trait ApplicationEnv: Any + Sync + Send {
    fn as_any(&self) -> &dyn Any;
}

pub trait ApplicationWorker: Any {
    fn as_any_mut(&mut self) -> &mut dyn Any;

    fn methods(&self) -> &Arc<HashMap<String, method::MethodDef>>;

    /// Passing copies of Server-global environment data to the worker.
    ///
    /// This is the first method called on each worker after spawning.
    fn absorb_env(
        &mut self,
        client: client::Client,
        config: Arc<conf::Config>,
        host_settings: Arc<sclient::HostSettings>,
        methods: Arc<HashMap<String, method::MethodDef>>,
        env: Box<dyn ApplicationEnv>,
    ) -> Result<(), String>;

    /// Called after absorb_env, but before any work occurs.
    fn worker_start(&mut self) -> Result<(), String>;

    /// Called for stateful sessions on CONNECT and for each request
    /// in a stateless session.
    fn start_session(&mut self) -> Result<(), String>;

    /// Called for stateful sessions on DISCONNECT or keepliave timeout,
    /// andcalled for stateless sessions (one-offs) after the single
    /// request has completed.
    fn end_session(&mut self) -> Result<(), String>;

    /// Called if the client sent a CONNECT but never sent a DISCONNECT
    /// within the configured timeout.
    fn keepalive_timeout(&mut self) -> Result<(), String>;

    /// Called on the worker when a MethodCall invocation exits with an Err.
    fn api_call_error(&mut self, request: &message::MethodCall, error: &str);

    /// Called every time our worker wakes up to check for signals,
    /// timeouts, etc.
    ///
    /// This method is only called when no other actions occur as
    /// a result of waking up.  It's not called if there is a
    /// shutdown signal, keepliave timeout, API request, etc.
    ///
    /// * `connected` - True if we are in the middle of a stateful conversation.
    fn worker_idle_wake(&mut self, connected: bool) -> Result<(), String>;

    /// Called after all work is done and the thread is going away.
    ///
    /// Offers a chance to clean up any resources.
    fn worker_end(&mut self) -> Result<(), String>;
}

pub trait Application {
    /// Application service name, e.g. opensrf.settings
    fn name(&self) -> &str;

    /// Called when a service first starts, just after connecting to OpenSRF.
    fn init(
        &mut self,
        client: client::Client,
        config: Arc<conf::Config>,
        host_settings: Arc<sclient::HostSettings>,
    ) -> Result<(), String>;

    /// Tell the server what methods this application implements.
    ///
    /// Called after self.init(), but before workers are spawned.
    fn register_methods(
        &self,
        client: client::Client,
        config: Arc<conf::Config>,
        host_settings: Arc<sclient::HostSettings>,
    ) -> Result<Vec<method::MethodDef>, String>;

    /// Returns a function pointer (ApplicationWorkerFactory) that returns
    /// new ApplicationWorker's when called.
    ///
    /// Dynamic trait objects cannot be passed to threads, but functions
    /// that generate them can.
    fn worker_factory(&self) -> fn() -> Box<dyn ApplicationWorker>;

    /// Creates a new application environment object.
    fn env(&self) -> Box<dyn ApplicationEnv>;
}
