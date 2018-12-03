extern crate dscfg_proto;
extern crate futures;
extern crate tokio_io;
extern crate serde;
extern crate serde_json;

pub use dscfg_proto::json;

//use tokio_io::{AsyncRead, AsyncWrite};
use futures::{Stream, Sink, Future};
use std::io;
use tokio_io::{AsyncRead, AsyncWrite};
use serde::{Serialize, Deserialize};

/// Error returned when DSCFG protocol fails.
#[derive(Debug)]
pub enum ProtocolError<E> {
    /// The response to the message wasn't expected
    UnexpectedResponse,
    /// The stream has ended before a message could be fully decoded.
    UnexpectedEof,
    /// Underlying communication error - e.g. I/O error.
    Communication(E),
}

/// DSCFG client
///
/// This represents a connection to the DSCFG server and allows
/// manipulating shared configurationn as well as receiving notifictions
/// about changes.
///
/// You should usually create it by calling `new()` function of this crate,
/// but you may use custom stream if you need finer control.
pub struct Client<C> {
    connection: C,
}

impl<Val: Serialize + for<'a> Deserialize<'a>, E, C: Stream<Item=dscfg_proto::Response<Val>, Error=E> + Sink<SinkItem=dscfg_proto::Request<Val>, SinkError=E>> Client<C> {
    /// Intantiates Client using provided custom `Stream + Sink`.
    pub fn custom(connection: C) -> Self {
        Client {
            connection,
        }
    }

    /// Sends request to set the `key` to given `value`.
    ///
    /// Returns future which resolves to `Client`, if the request succeeded.
    pub fn set_value(self, key: String, value: Val) -> impl Future<Item=Self, Error=E> {
        self.connection.send(dscfg_proto::Request::Set { key, value, })
            .map(|connection| Client { connection, })
    }

    /// Sends request for getting value of given key and waits for the answer.
    ///
    /// Returns future which resolves to `(Val, Self)` if successful.
    pub fn get_value<K: Into<String>>(self, key: K) -> impl Future<Item=(Val, Self), Error=ProtocolError<E>> {
        self.connection.send(dscfg_proto::Request::Get { key: key.into() })
            .and_then(|connection| connection.into_future().map_err(|(err, _)| err))
            .map_err(ProtocolError::Communication)
            .and_then(|(result, connection)| {
                match result {
                    Some(dscfg_proto::Response::Value { key: _, value, }) => Ok((value, Client { connection, })),
                    None => Err(ProtocolError::UnexpectedEof),
                    _ => Err(ProtocolError::UnexpectedResponse),
                }
            })
    }

    /// Subscribes for notifications of changes of value of specified `key`
    pub fn listen_notifications<K: Into<String>>(self, key: K, notify_now: bool) -> impl Stream<Item=(String, Val), Error=E> {
        self.connection
            .send(dscfg_proto::Request::Subscribe { key: key.into(), notify_now, })
            .and_then(|s| s.into_future().map_err(|(err, _)| err))
            .map(|(_, s)| s)
            .flatten_stream()
            .filter_map(|msg| match msg {
                dscfg_proto::Response::Value { key, value } => Some((key, value)),
                _ => None,
            })
    }
}

/// Creates a dscfg client that encodes communication as length-delimited Json messages.
pub fn new<Val: Serialize + for<'a> Deserialize<'a>, C: AsyncRead + AsyncWrite>(connection: C) -> Client<impl Stream<Item=dscfg_proto::Response<Val>, Error=io::Error> + Sink<SinkItem=dscfg_proto::Request<Val>, SinkError=io::Error>> {
    // The deprecation message suggests to use `tokio-codec` instead,
    // which doesn't actually implement it, and depending on `tokio::codec`
    // just pulls in too many dependencies.
    #[allow(deprecated)]
    let client = tokio_io::codec::length_delimited::Builder::new()
        .native_endian()
        .new_framed(connection)
        .and_then(|message| serde_json::from_slice(&message).map_err(Into::into))
        .with(|message| serde_json::to_vec(&message).map_err(io::Error::from));

    Client::custom(client)
}
