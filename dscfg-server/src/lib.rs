extern crate futures;
extern crate tokio_io;
extern crate dscfg_proto;
extern crate void;
extern crate same;
#[macro_use]
extern crate slog;
extern crate serde_json;

pub use dscfg_proto::json;

use futures::sync::mpsc::{self, UnboundedSender};
use futures::{Future, Stream, Sink};
use futures::future;
use void::Void;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use same::RefCmp;
use std::sync::RwLock;
use std::io;

#[derive(Clone)]
struct Subscriptions(Arc<RwLock<HashMap<String, HashSet<RefCmp<Arc<mpsc::UnboundedSender<dscfg_proto::Response>>>>>>>);

impl Subscriptions {
    fn new() -> Self {
        Subscriptions(Default::default())
    }

    fn subscribe(&self, client: &Arc<mpsc::UnboundedSender<dscfg_proto::Response>>, key: String) -> bool {
        let mut subscriptions = self.0.write().unwrap();
        let client = RefCmp(Arc::clone(client));

        subscriptions.entry(key).or_insert_with(HashSet::new).insert(client)
    }

    fn unsubscribe(&self, client: &Arc<mpsc::UnboundedSender<dscfg_proto::Response>>, key: &str) -> bool {
        let mut subscriptions = self.0.write().unwrap();
        let client = RefCmp(Arc::clone(client));

        if let Some(subscriptions) = subscriptions.get_mut(key) {
            subscriptions.remove(&client)
        } else {
            false
        }
    }

    fn unsubscribe_all(&self, client: &Arc<mpsc::UnboundedSender<dscfg_proto::Response>>) {
        let mut subscriptions = self.0.write().unwrap();
        let client = RefCmp(Arc::clone(client));

        for (_, subscriptions) in &mut *subscriptions {
            subscriptions.remove(&client);
        }
    }

    fn broadcast(&self, key: String, value: json::Value) {
        use dscfg_proto::Response;

        let subscriptions = self.0.read().unwrap();

        if let Some(subscriptions) = subscriptions.get(&key) {
            for subscription in subscriptions {
                subscription
                    .unbounded_send(Response::Value { key: key.clone(), value: value.clone() })
                    // This should never happen as the client unregisters itself.
                    .unwrap()
            }
        }
    }
}

/// A trait for errors to tell whether they are fatal.
///
/// This is used for determining whether the server should continue runnin or stop.
pub trait IsFatalError {
    /// Returns `true` if all future operations will (very likely) fail.
    fn is_fatal(&self) -> bool;
}

impl IsFatalError for Void {
    fn is_fatal(&self) -> bool {
        match *self {}
    }
}

/// Specification of interface for accessing configuration.
///
/// The configuration can be stored using many different methods. In order to implement a new way
/// of storing configuration data, you must implement this trait for your type.
pub trait Storage {
    /// Error which may occur when attempting to write to the storage.
    type SetError: IsFatalError;
    /// Error which may occur when attempting to read from the storage.
    type GetError: IsFatalError;

    /// When this function is called, the implementor must store the given value for key in the
    /// storage or return error in case of failure.
    fn set(&mut self, key: String, value: json::Value) -> Result<(), Self::SetError>;

    /// The implementor must return the value at given key (if exists, `None` if not) or error if getting fails.
    fn get(&mut self, key: &str) -> Result<Option<json::Value>, Self::GetError>;
}

impl<T: Storage + ?Sized> Storage for Box<T> {
    type SetError = T::SetError;
    type GetError = T::GetError;

    fn set(&mut self, key: String, value: json::Value) -> Result<(), Self::SetError> {
        (**self).set(key, value)
    }

    fn get(&mut self, key: &str) -> Result<Option<json::Value>, Self::GetError> {
        (**self).get(key)
    }
}

/// Error that might occur when accessing `Store` synchronized with mutex.
pub enum SyncOpResult<T> {
    /// The mutex was poisoned
    Poisoned,
    /// The underlying implementation failed.
    Other(T),
}

impl<T: IsFatalError> IsFatalError for SyncOpResult<T> {
    fn is_fatal(&self) -> bool {
        match self {
            SyncOpResult::Poisoned => true,
            SyncOpResult::Other(err) => err.is_fatal(),
        }
    }
}

impl<T> Storage for Arc<Mutex<T>> where T: Storage + ?Sized {
    type SetError = SyncOpResult<T::SetError>;
    type GetError = SyncOpResult<T::GetError>;

    fn set(&mut self, key: String, value: json::Value) -> Result<(), Self::SetError> {
        self.lock()
            .map_err(|_| SyncOpResult::Poisoned)?
            .set(key, value)
            .map_err(SyncOpResult::Other)
    }

    fn get(&mut self, key: &str) -> Result<Option<json::Value>, Self::GetError> {
        self.lock()
            .map_err(|_| SyncOpResult::Poisoned)?
            .get(key)
            .map_err(SyncOpResult::Other)
    }
}

/// Parameters the server needs to run
///
/// Since there are several parameters the server needs, it's better
/// to pass them as struct containing them.
pub struct ServerParams<Incoming, Store, Executor, Logger> where 
    Incoming: Stream,
    Store: Storage + Clone + Send,
    Executor: future::Executor<Box<'static + Future<Item=(), Error=()> + Send>>,
    Logger: Into<slog::Logger> {

    /// Clients that are accepted.
    pub incoming_clients: Incoming,
    /// The implementation of configuration storage.
    pub storage: Store,
    /// Futures executor used for handling the clients. 
    pub executor: Executor,
    /// `slog` Logger used for logging.
    pub logger: Logger,
}

/// This struct can be used in place of logger to discard all logs.
pub struct DiscardLogs;

impl From<DiscardLogs> for slog::Logger {
    fn from(_: DiscardLogs) -> Self {
        slog::Logger::root(slog::Discard, o!())
    }
}

/// Error that might occur when attempting to accept a connection.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum HandlingError<E> {
    /// Accepting failed.
    AcceptError(E),
    /// The server is stopping.
    Shutdown,
}

fn handle_client<Client, Store, Error>(client: Client, subscriptions: Subscriptions, mut storage: Store, canceler: UnboundedSender<()>) -> Box<'static + Future<Item=(), Error=()> + Send> where
    Client: 'static + Stream<Item=dscfg_proto::Request, Error=Error> + Sink<SinkItem=dscfg_proto::Response, SinkError=Error> + Send,
    Store: 'static + Storage + Send,
    Error: 'static {

    use dscfg_proto::{Request, Response};

    let (sender, receiver) = mpsc::unbounded();
    let sender = Arc::new(sender);
    let unsubscriber = subscriptions.clone();
    let sender_unsubscribe = sender.clone();

    let (sink, stream) = client.split();

    let stream = stream
        .map(move |request| {
            match request {
                Request::Set { key, value } => {
                    match storage.set(key.clone(), value.clone()) {
                        Ok(_) => {
                            subscriptions.broadcast(key, value);
                            Response::OperationOk
                        },
                        Err(err) => {
                            if err.is_fatal() {
                                let _ = canceler.unbounded_send(());
                            }
                            Response::OperationFailed
                        },
                    }
                },
                Request::Get { key } => {
                    match storage.get(&key) {
                        Ok(Some(value)) => {
                            Response::Value { key, value }
                        },
                        Ok(None) => Response::Value { key, value: json::Value::Null },
                        Err(err) => {
                            if err.is_fatal() {
                                let _ = canceler.unbounded_send(());
                            }
                            Response::OperationFailed
                        },
                    }
                },
                Request::Subscribe { key, notify_now } => {
                    if notify_now {
                        match storage.get(&key) {
                            Ok(Some(value)) => {
                                let notification = Response::Value {
                                    key: key.clone(),
                                    value: value,
                                };
                                sender.unbounded_send(notification).unwrap();
                            },
                            Ok(None) => {
                                let notification = Response::Value {
                                    key: key.clone(),
                                    value: json::Value::Null,
                                };
                                sender.unbounded_send(notification).unwrap();
                            },
                            Err(_) => return Response::OperationFailed,
                        }
                    }

                    if subscriptions.subscribe(&sender, key) {
                        Response::OperationOk
                    } else {
                        Response::Ignored
                    }
                },
                Request::Unsubscribe { key } => {
                    if subscriptions.unsubscribe(&sender, &key) {
                        Response::OperationOk
                    } else {
                        Response::Ignored
                    }
                }
            }
        })
        .map_err(std::mem::drop);

    let receiver = receiver.map_err(|_| panic!("sender terminated"));

    let sink = sink.sink_map_err(std::mem::drop);

    Box::new(stream
        .select(receiver)
        .forward(sink)
        .map(std::mem::drop)
        .map_err(std::mem::drop)
        .then(move |result| { unsubscriber.unsubscribe_all(&sender_unsubscribe); result })
    )
}

/// Creates a server with custom client stream.
///
/// This may be used if one wants control over how the messages are serialized.
/// If you want to use the default serialization (length-delimited json encoding),
/// use `serve()` function.
pub fn custom<Incoming, Store, Executor, Logger, CommError>(server_params: ServerParams<Incoming, Store, Executor, Logger>) -> impl Future<Item=(), Error=HandlingError<Incoming::Error>> where
    Incoming: Stream,
    Incoming::Item: 'static + Stream<Item=dscfg_proto::Request, Error=CommError> + Sink<SinkItem=dscfg_proto::Response, SinkError=CommError> + Send,
    Store: 'static + Storage + Clone + Send,
    Executor: future::Executor<Box<'static + Future<Item=(), Error=()> + Send>>,
    Logger: Into<slog::Logger>,
    CommError: 'static {

    let logger = server_params.logger.into();
    let executor = server_params.executor;
    let storage = server_params.storage;

    let subscriptions = Subscriptions::new();
    let (canceler, cancelable) = mpsc::unbounded();

    let cancelable = cancelable
        .into_future()
        .then(|_: Result<(Option<()>, futures::sync::mpsc::UnboundedReceiver<()>), _>| -> Result<(), HandlingError<Incoming::Error>> { Ok(()) });

    let server = server_params.incoming_clients
        .map_err(HandlingError::AcceptError)
        .for_each(move |client| {
            let client = handle_client(client, subscriptions.clone(), storage.clone(), canceler.clone());

            match executor.execute(client) {
                Ok(_) => Ok(()),
                Err(ref err) if err.kind() == future::ExecuteErrorKind::NoCapacity => {
                    error!(logger, "failed to handle the client"; "cause" => "no capacity");
                    // We'll just ignore this client - others might work
                    Ok(())
                },
                Err(_) => {
                    info!(logger, "shutting down");
                    Err(HandlingError::Shutdown)
                },
            }
        })
        .select(cancelable)
        .map(std::mem::drop)
        .map_err(|(e, _)| e);
    server
}

/// Creates default dscfg server.
///
/// This server uses length-delimited Json messages to transfer the data. Use `custom()` if you
/// want to control encoding.
pub fn serve<Incoming, Store, Executor, Logger>(server_params: ServerParams<Incoming, Store, Executor, Logger>) -> impl Future<Item=(), Error=HandlingError<Incoming::Error>> where
    Incoming: Stream,
    Incoming::Item: 'static + tokio_io::AsyncRead + tokio_io::AsyncWrite + Send,
    Store: 'static + Storage + Clone + Send,
    Executor: future::Executor<Box<'static + Future<Item=(), Error=()> + Send>>,
    Logger: Into<slog::Logger> {

    let incoming_clients = server_params.incoming_clients.map(move |stream| {
        // Workaround for unsuitable deprecation message - see
        // https://github.com/tokio-rs/tokio/issues/680
        #[allow(deprecated)]
        tokio_io::codec::length_delimited::Builder::new()
            .native_endian()
            .new_framed(stream)
            .and_then(|message| serde_json::from_slice(&message).map_err(Into::into))
            .with(|message| serde_json::to_vec(&message).map_err(io::Error::from))
    });

    let params = ServerParams {
        incoming_clients,
        storage: server_params.storage,
        executor: server_params.executor,
        logger: server_params.logger
    };
    custom(params)
}
