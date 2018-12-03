//! Protocol specification for dynamic shared configuration
//!
//! This crate contains types used in both client and server,
//! so they aren't duplicated. This ensures consistency - if
//! a change is made, it will immediately affect both client and the
//! server.
//!
//! Features of this crate:
//!
//! * `client` - derives traits needed by the client
//! * `server` - derives traits needed by the server
//!
//! The purpose of having those features is to not slow down
//! compilation by compiling unneeded code as well as not increase
//! the binary size unneccessarily.

pub extern crate serde_json;
extern crate serde;
#[macro_use]
extern crate serde_derive;

/// Reexport for `serde_json`
///
/// This is mainly useful to avoid having to specify another dependency.
/// It's also somewhat nicer to write `json::Value` instead of `serde_json::Value`.
pub mod json {
    pub use ::serde_json::*;
}

/// Request sent from client to server.
///
/// This enum represents possible requests accepted by the server.
/// See the documentation of its variants to understand possible requests.
///
/// This enum is parametric over value type in order to skip conversions
/// to `json::Value` when sending the request.
#[cfg_attr(feature = "client", derive(Serialize))]
#[cfg_attr(feature = "server", derive(Deserialize))]
pub enum Request<Val = json::Value> {
    /// Sets the value of `key` to `value`
    ///
    /// There's no response, but if the client is subscribed
    /// with the `key`, it will get the notification.
    Set { key: String, value: Val },

    /// Gets the value of the `key`
    ///
    /// Response of type `Value` follows this request.
    Get { key: String },

    /// Requests notifications when any of the keys change.
    ///
    /// If `notify_now` is set to `true`, the client is
    /// also notified immediately after the response.
    ///
    /// The response is either `OperationOk`, if the cliet was
    /// subscribed or `Ignored`, if the client was already subscribed.
    Subscribe { key: String, notify_now: bool },

    /// Requests the server to stop notifying the client
    ///
    /// Note that this isn't necessary if the client is going to
    /// disconnect - the subscribtions of the client are automatically
    /// cleared on disconnect.
    ///
    /// If the unsubscribe operation was performed, `OperationOk`
    /// response is sent. If the client wasn't subscribed,
    /// `Ignored` is sent.
    Unsubscribe { key: String },
}

/// Response or notification sent to the client.
#[cfg_attr(feature = "server", derive(Serialize))]
#[cfg_attr(feature = "client", derive(Deserialize))]
pub enum Response<Val = json::Value> {
    /// Informs the client about the value for certain key.
    Value { key: String, value: Val },

    /// Informs the client that the operation was performed.
    OperationOk,

    /// Informs the client that the operation failed.
    OperationFailed,

    /// Informs the client that the operation didn't have to be
    /// performed.
    ///
    /// This happens if the client attempts to subscribe to key
    /// he's already subscribed to, or unsubscribe from key he
    /// isn't subscribed to.
    Ignored,
}
