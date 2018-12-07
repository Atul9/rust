#![doc = "This file was automatically generated by the varlink rust generator"]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
use failure::{Backtrace, Context, Fail};
use serde_derive::{Deserialize, Serialize};
use serde_json;
use std::io::BufRead;
use std::sync::{Arc, RwLock};
use varlink::{self, CallTrait};
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct PingError_Args {
    pub parameter: i64,
}
pub trait VarlinkCallError: varlink::CallTrait {
    fn reply_ping_error(&mut self, parameter: i64) -> varlink::Result<()> {
        self.reply_struct(varlink::Reply::error(
            "org.example.ping.PingError",
            Some(serde_json::to_value(PingError_Args { parameter })?),
        ))
    }
}
impl<'a> VarlinkCallError for varlink::Call<'a> {}
#[derive(Debug)]
pub struct Error {
    inner: Context<ErrorKind>,
}
#[derive(Clone, PartialEq, Debug, Fail)]
pub enum ErrorKind {
    #[fail(display = "IO error")]
    Io_Error(::std::io::ErrorKind),
    #[fail(display = "(De)Serialization Error")]
    SerdeJson_Error(serde_json::error::Category),
    #[fail(display = "Varlink Error")]
    Varlink_Error(varlink::ErrorKind),
    #[fail(display = "Unknown error reply: '{:#?}'", _0)]
    VarlinkReply_Error(varlink::Reply),
    #[fail(display = "org.example.ping.PingError: {:#?}", _0)]
    PingError(Option<PingError_Args>),
}
impl Fail for Error {
    fn cause(&self) -> Option<&Fail> {
        self.inner.cause()
    }
    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}
impl ::std::fmt::Display for Error {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        ::std::fmt::Display::fmt(&self.inner, f)
    }
}
impl Error {
    #[allow(dead_code)]
    pub fn kind(&self) -> ErrorKind {
        self.inner.get_context().clone()
    }
}
impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Error {
        Error {
            inner: Context::new(kind),
        }
    }
}
impl From<Context<ErrorKind>> for Error {
    fn from(inner: Context<ErrorKind>) -> Error {
        Error { inner }
    }
}
impl From<::std::io::Error> for Error {
    fn from(e: ::std::io::Error) -> Error {
        let kind = e.kind();
        e.context(ErrorKind::Io_Error(kind)).into()
    }
}
impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Error {
        let cat = e.classify();
        e.context(ErrorKind::SerdeJson_Error(cat)).into()
    }
}
#[allow(dead_code)]
pub type Result<T> = ::std::result::Result<T, Error>;
impl From<varlink::Error> for Error {
    fn from(e: varlink::Error) -> Self {
        let kind = e.kind();
        match kind {
            varlink::ErrorKind::Io(kind) => e.context(ErrorKind::Io_Error(kind)).into(),
            varlink::ErrorKind::SerdeJsonSer(cat) => {
                e.context(ErrorKind::SerdeJson_Error(cat)).into()
            }
            kind => e.context(ErrorKind::Varlink_Error(kind)).into(),
        }
    }
}
impl From<varlink::Reply> for Error {
    fn from(e: varlink::Reply) -> Self {
        if varlink::Error::is_error(&e) {
            return varlink::Error::from(e).into();
        }
        match e {
            varlink::Reply {
                error: Some(ref t), ..
            } if t == "org.example.ping.PingError" => match e {
                varlink::Reply {
                    parameters: Some(p),
                    ..
                } => match serde_json::from_value(p) {
                    Ok(v) => ErrorKind::PingError(v).into(),
                    Err(_) => ErrorKind::PingError(None).into(),
                },
                _ => ErrorKind::PingError(None).into(),
            },
            _ => ErrorKind::VarlinkReply_Error(e).into(),
        }
    }
}
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Ping_Reply {
    pub pong: String,
}
impl varlink::VarlinkReply for Ping_Reply {}
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Ping_Args {
    pub ping: String,
}
pub trait Call_Ping: VarlinkCallError {
    fn reply(&mut self, pong: String) -> varlink::Result<()> {
        self.reply_struct(Ping_Reply { pong }.into())
    }
}
impl<'a> Call_Ping for varlink::Call<'a> {}
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Upgrade_Reply {}
impl varlink::VarlinkReply for Upgrade_Reply {}
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Upgrade_Args {}
pub trait Call_Upgrade: VarlinkCallError {
    fn reply(&mut self) -> varlink::Result<()> {
        self.reply_struct(varlink::Reply::parameters(None))
    }
}
impl<'a> Call_Upgrade for varlink::Call<'a> {}
pub trait VarlinkInterface {
    fn ping(&self, call: &mut Call_Ping, ping: String) -> varlink::Result<()>;
    fn upgrade(&self, call: &mut Call_Upgrade) -> varlink::Result<()>;
    fn call_upgraded(
        &self,
        _call: &mut varlink::Call,
        _bufreader: &mut BufRead,
    ) -> varlink::Result<Vec<u8>> {
        Ok(Vec::new())
    }
}
pub trait VarlinkClientInterface {
    fn ping(&mut self, ping: String) -> varlink::MethodCall<Ping_Args, Ping_Reply, Error>;
    fn upgrade(&mut self) -> varlink::MethodCall<Upgrade_Args, Upgrade_Reply, Error>;
}
#[allow(dead_code)]
pub struct VarlinkClient {
    connection: Arc<RwLock<varlink::Connection>>,
}
impl VarlinkClient {
    #[allow(dead_code)]
    pub fn new(connection: Arc<RwLock<varlink::Connection>>) -> Self {
        VarlinkClient { connection }
    }
}
impl VarlinkClientInterface for VarlinkClient {
    fn ping(&mut self, ping: String) -> varlink::MethodCall<Ping_Args, Ping_Reply, Error> {
        varlink::MethodCall::<Ping_Args, Ping_Reply, Error>::new(
            self.connection.clone(),
            "org.example.ping.Ping",
            Ping_Args { ping },
        )
    }
    fn upgrade(&mut self) -> varlink::MethodCall<Upgrade_Args, Upgrade_Reply, Error> {
        varlink::MethodCall::<Upgrade_Args, Upgrade_Reply, Error>::new(
            self.connection.clone(),
            "org.example.ping.Upgrade",
            Upgrade_Args {},
        )
    }
}
#[allow(dead_code)]
pub struct VarlinkInterfaceProxy {
    inner: Box<VarlinkInterface + Send + Sync>,
}
#[allow(dead_code)]
pub fn new(inner: Box<VarlinkInterface + Send + Sync>) -> VarlinkInterfaceProxy {
    VarlinkInterfaceProxy { inner }
}
impl varlink::Interface for VarlinkInterfaceProxy {
    fn get_description(&self) -> &'static str {
        "# Example service\ninterface org.example.ping\n\n# Returns the same string\nmethod Ping(ping: string) -> (pong: string)\n\nmethod Upgrade() -> ()\n\nerror PingError(parameter: int)"
    }
    fn get_name(&self) -> &'static str {
        "org.example.ping"
    }
    fn call_upgraded(
        &self,
        call: &mut varlink::Call,
        bufreader: &mut BufRead,
    ) -> varlink::Result<Vec<u8>> {
        self.inner.call_upgraded(call, bufreader)
    }
    fn call(&self, call: &mut varlink::Call) -> varlink::Result<()> {
        let req = call.request.unwrap();
        match req.method.as_ref() {
            "org.example.ping.Ping" => {
                if let Some(args) = req.parameters.clone() {
                    let args: Ping_Args = match serde_json::from_value(args) {
                        Ok(v) => v,
                        Err(e) => {
                            let es = format!("{}", e);
                            let _ = call.reply_invalid_parameter(es.clone());
                            return Err(varlink::ErrorKind::SerdeJsonDe(es).into());
                        }
                    };
                    self.inner.ping(call as &mut Call_Ping, args.ping)
                } else {
                    call.reply_invalid_parameter("parameters".into())
                }
            }
            "org.example.ping.Upgrade" => self.inner.upgrade(call as &mut Call_Upgrade),
            m => call.reply_method_not_found(String::from(m)),
        }
    }
}
