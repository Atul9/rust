//! Handle network connections for a varlink service

use {ErrorKind, Result};
//#![feature(getpid)]
//use std::process;
// FIXME
use libc::getpid;
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::os::unix::io::{FromRawFd, IntoRawFd};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::RwLock;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
// FIXME: abstract unix domains sockets still not in std
// FIXME: https://github.com/rust-lang/rust/issues/14194
use unix_socket::UnixListener as AbstractUnixListener;

#[derive(Debug)]
enum VarlinkListener {
    TCP(Option<TcpListener>, bool),
    UNIX(Option<UnixListener>, bool),
}

#[derive(Debug)]
enum VarlinkStream {
    TCP(TcpStream),
    UNIX(UnixStream),
}

impl<'a> VarlinkStream {
    pub fn split(&mut self) -> Result<(Box<Read + Send + Sync>, Box<Write + Send + Sync>)> {
        match *self {
            VarlinkStream::TCP(ref mut s) => {
                Ok((Box::new(s.try_clone()?), Box::new(s.try_clone()?)))
            }
            VarlinkStream::UNIX(ref mut s) => {
                Ok((Box::new(s.try_clone()?), Box::new(s.try_clone()?)))
            }
        }
    }
    pub fn shutdown(&mut self) -> Result<()> {
        match *self {
            VarlinkStream::TCP(ref mut s) => s.shutdown(Shutdown::Both)?,
            VarlinkStream::UNIX(ref mut s) => s.shutdown(Shutdown::Both)?,
        }
        Ok(())
    }
}

fn activation_listener() -> Result<Option<i32>> {
    let nfds: u32;

    match env::var("LISTEN_FDS") {
        Ok(ref n) => match n.parse::<u32>() {
            Ok(n) if n >= 1 => nfds = n,
            _ => return Ok(None),
        },
        _ => return Ok(None),
    }

    unsafe {
        match env::var("LISTEN_PID") {
            //FIXME: replace Ok(getpid()) with Ok(process::id())
            Ok(ref pid) if pid.parse::<i32>() == Ok(getpid()) => {}
            _ => return Ok(None),
        }
    }

    if nfds == 1 {
        return Ok(Some(3));
    }

    let fdnames: String;

    match env::var("LISTEN_FDNAMES") {
        Ok(n) => {
            fdnames = n;
        }
        _ => return Ok(None),
    }

    for (i, v) in fdnames.split(":").enumerate() {
        if v == "varlink" {
            return Ok(Some(3 + i as i32));
        }
    }

    Ok(None)
}

//FIXME: add Drop with shutdown() and unix file removal
impl VarlinkListener {
    pub fn new<S: ?Sized + AsRef<str>>(address: &S, timeout: u64) -> Result<Self> {
        let address = address.as_ref();
        if let Some(l) = activation_listener()? {
            if address.starts_with("tcp:") {
                unsafe {
                    let s = TcpStream::from_raw_fd(l);
                    if timeout != 0 {
                        s.set_read_timeout(Some(Duration::from_secs(timeout)))?;
                    }
                    return Ok(VarlinkListener::TCP(
                        Some(TcpListener::from_raw_fd(s.into_raw_fd())),
                        true,
                    ));
                }
            } else if address.starts_with("unix:") {
                unsafe {
                    let s = UnixStream::from_raw_fd(l);
                    if timeout != 0 {
                        s.set_read_timeout(Some(Duration::from_secs(timeout)))?;
                    }
                    return Ok(VarlinkListener::UNIX(
                        Some(UnixListener::from_raw_fd(s.into_raw_fd())),
                        true,
                    ));
                }
            } else {
                return Err(ErrorKind::InvalidAddress.into());
            }
        }

        if address.starts_with("tcp:") {
            let l = TcpListener::bind(&address[4..])?;
            unsafe {
                let s = TcpStream::from_raw_fd(l.into_raw_fd());
                if timeout != 0 {
                    s.set_read_timeout(Some(Duration::from_secs(timeout)))?;
                }
                Ok(VarlinkListener::TCP(
                    Some(TcpListener::from_raw_fd(s.into_raw_fd())),
                    false,
                ))
            }
        } else if address.starts_with("unix:") {
            let mut addr = String::from(address[5..].split(";").next().unwrap());
            if addr.starts_with("@") {
                addr = addr.replacen("@", "\0", 1);
                let l = AbstractUnixListener::bind(addr)?;
                unsafe {
                    let s = UnixStream::from_raw_fd(l.into_raw_fd());
                    if timeout != 0 {
                        s.set_read_timeout(Some(Duration::from_secs(timeout)))?;
                    }
                    return Ok(VarlinkListener::UNIX(
                        Some(UnixListener::from_raw_fd(s.into_raw_fd())),
                        false,
                    ));
                }
            }
            // ignore error on non-existant file
            let _ = fs::remove_file(&*addr);
            let l = UnixListener::bind(addr)?;
            unsafe {
                let s = UnixStream::from_raw_fd(l.into_raw_fd());
                if timeout != 0 {
                    s.set_read_timeout(Some(Duration::from_secs(timeout)))?;
                }
                Ok(VarlinkListener::UNIX(
                    Some(UnixListener::from_raw_fd(s.into_raw_fd())),
                    false,
                ))
            }
        } else {
            Err(ErrorKind::InvalidAddress.into())
        }
    }

    pub fn accept(&self) -> Result<VarlinkStream> {
        match self {
            &VarlinkListener::TCP(Some(ref l), _) => {
                let (mut s, _addr) = l.accept()?;
                Ok(VarlinkStream::TCP(s))
            }
            &VarlinkListener::UNIX(Some(ref l), _) => {
                let (mut s, _addr) = l.accept()?;
                Ok(VarlinkStream::UNIX(s))
            }
            _ => Err(ErrorKind::ConnectionClosed.into()),
        }
    }
    pub fn set_nonblocking(&self, b: bool) -> Result<()> {
        match self {
            &VarlinkListener::TCP(Some(ref l), _) => l.set_nonblocking(b)?,
            &VarlinkListener::UNIX(Some(ref l), _) => l.set_nonblocking(b)?,
            _ => Err(ErrorKind::ConnectionClosed)?,
        }
        Ok(())
    }
}

impl Drop for VarlinkListener {
    fn drop(&mut self) {
        match *self {
            VarlinkListener::UNIX(Some(ref listener), false) => {
                if let Ok(local_addr) = listener.local_addr() {
                    if let Some(path) = local_addr.as_pathname() {
                        let _ = fs::remove_file(path);
                    }
                }
            }
            VarlinkListener::UNIX(ref mut listener, true) => {
                if let Some(l) = listener.take() {
                    unsafe {
                        let s = UnixStream::from_raw_fd(l.into_raw_fd());
                        let _ = s.set_read_timeout(None);
                    }
                }
            }
            _ => {}
        }
    }
}

enum Message {
    NewJob(Job),
    Terminate,
}

struct ThreadPool {
    workers: Vec<Worker>,
    num_busy: Arc<RwLock<i64>>,
    sender: mpsc::Sender<Message>,
}

trait FnBox {
    fn call_box(self: Box<Self>);
}

impl<F: FnOnce()> FnBox for F {
    fn call_box(self: Box<F>) {
        (*self)()
    }
}

type Job = Box<FnBox + Send + 'static>;

impl ThreadPool {
    /// Create a new ThreadPool.
    ///
    /// The size is the number of threads in the pool.
    ///
    /// # Panics
    ///
    /// The `new` function will panic if the size is zero.
    pub fn new(size: usize) -> ThreadPool {
        assert!(size > 0);

        let (sender, receiver) = mpsc::channel();

        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(size);

        let num_busy = Arc::new(RwLock::new(0 as i64));

        for _ in 0..size {
            workers.push(Worker::new(Arc::clone(&receiver), Arc::clone(&num_busy)));
        }

        ThreadPool {
            workers,
            sender,
            num_busy,
        }
    }

    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);

        self.sender.send(Message::NewJob(job)).unwrap();
    }

    pub fn num_busy(&self) -> i64 {
        let num_busy = self.num_busy.read().unwrap();
        *num_busy
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        for _ in &mut self.workers {
            self.sender.send(Message::Terminate).unwrap();
        }

        for worker in &mut self.workers {
            if let Some(thread) = worker.thread.take() {
                thread.join().unwrap();
            }
        }
    }
}

struct Worker {
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    fn new(receiver: Arc<Mutex<mpsc::Receiver<Message>>>, num_busy: Arc<RwLock<i64>>) -> Worker {
        let thread = thread::spawn(move || loop {
            let message = receiver.lock().unwrap().recv().unwrap();

            match message {
                Message::NewJob(job) => {
                    {
                        let mut num_busy = num_busy.write().unwrap();
                        *num_busy += 1;
                    }
                    job.call_box();
                    {
                        let mut num_busy = num_busy.write().unwrap();
                        *num_busy -= 1;
                    }
                }
                Message::Terminate => {
                    break;
                }
            }
        });

        Worker {
            thread: Some(thread),
        }
    }
}

/// `listen` creates a server, with `num_worker` threads listening on `varlink_uri`.
///
/// If an `accept_timeout` != 0 is specified, this function returns after the specified
/// amount of seconds, if no new connection is made in that time frame. It still waits for
/// all pending connections to finish.
///
///# Examples
///
///```
/// extern crate failure;
/// extern crate varlink;
/// use failure::Fail;
///
/// let service = varlink::VarlinkService::new(
///     "org.varlink",
///     "test service",
///     "0.1",
///     "http://varlink.org",
///     vec![/* Your varlink interfaces go here */],
/// );
///
/// if let Err(e) = varlink::listen(service, "unix:/tmp/test_listen_timeout", 10, 1) {
///     if e.kind() != varlink::ErrorKind::Timeout {
///         panic!("Error listen: {:?}", e.cause());
///     }
/// }
///```
///# Note
/// You don't have to use this simple server. With the `VarlinkService::handle()` method you
/// can implement your own server model using whatever framework you prefer.
pub fn listen<S: ?Sized + AsRef<str>>(
    service: ::VarlinkService,
    address: &S,
    workers: usize,
    accept_timeout: u64,
) -> Result<()> {
    let service = Arc::new(service);
    let listener = Arc::new(VarlinkListener::new(address, accept_timeout)?);
    listener.set_nonblocking(false)?;
    let pool = ThreadPool::new(workers);

    loop {
        let mut stream: VarlinkStream = match listener.accept() {
            Err(ref e) if e.kind() == ErrorKind::Io(::std::io::ErrorKind::WouldBlock) => {
                if pool.num_busy() == 0 {
                    return Err(ErrorKind::Timeout.into());
                }
                continue;
            }
            r => r?,
        };
        let service = service.clone();

        pool.execute(move || {
            let (mut r, mut w) = stream.split().expect("Could not split stream");

            if let Err(_e) = service.handle(&mut r, &mut w) {
                //println!("Handle Error: {}", e);
                let _ = stream.shutdown();
            }
        });
    }
}
