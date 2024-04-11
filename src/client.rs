use std::thread::JoinHandle;

use log::trace;
use necronomicon::{full_decode, Owned, Packet, Pool};

pub struct Builder<P, R, W>
where
    P: Pool,
    R: FnMut(Packet<<P::Buffer as Owned>::Shared>) -> bool,
    W: FnMut(P) -> Packet<<P::Buffer as Owned>::Shared>,
{
    reader_pool: P,
    reader_service: R,

    writer_pool: P,
    writer_service: W,
}

impl<P, R, W> Builder<P, R, W>
where
    P: Pool,
    R: FnMut(Packet<<P::Buffer as Owned>::Shared>) -> bool,
    W: FnMut(P) -> Packet<<P::Buffer as Owned>::Shared>,
{
    pub fn new(reader_pool: P, reader_service: R, writer_pool: P, writer_service: W) -> Self {
        Self {
            reader_pool,
            reader_service,
            writer_pool,
            writer_service,
        }
    }

    pub fn connect(
        self,
        addr: impl std::net::ToSocketAddrs,
    ) -> Result<(Receiver<P, R>, Sender), std::io::Error> {
        let stream = std::net::TcpStream::connect(addr)?;

        Ok((
            Receiver::new(stream.try_clone()?, self.reader_pool, self.reader_service),
            Sender::new(stream),
        ))
    }
}

pub struct Receiver<P, R>
where
    P: Pool,
    R: FnMut(Packet<<P::Buffer as Owned>::Shared>) -> bool,
{
    stream: std::net::TcpStream,

    pool: P,
    service: R,
}

impl<P, R> Receiver<P, R>
where
    P: Pool,
    R: FnMut(Packet<<P::Buffer as Owned>::Shared>) -> bool,
{
    fn new(stream: std::net::TcpStream, pool: P, service: R) -> Self {
        Self {
            stream,
            pool,
            service,
        }
    }

    pub fn run(self) -> JoinHandle<()> {
        let Self {
            stream,
            pool,
            service,
        } = self;

        std::thread::spawn(move || {
            let mut previous_decoded_header = None;

            'pool: loop {
                let buffer = pool.acquire();
                match buffer {
                    Ok(mut buffer) => {
                        'decode: loop {
                            // We should know whether we have enough buffer to read the packet or not by checking the header.
                            // BUT, that also means we need to keep the header around and not just discard it. That way we can read
                            // the rest of the packet... so we need to change the `full_decode` to take an optional header.
                            match full_decode(
                                &mut stream,
                                &mut buffer,
                                previous_decoded_header.take(),
                            ) {
                                Ok(packet) => {
                                    trace!("got {:?} packet", packet);

                                    if !service(packet) {
                                        break 'pool;
                                    }
                                }
                                Err(necronomicon::Error::BufferTooSmallForPacketDecode {
                                    header,
                                    ..
                                }) => {
                                    let _ = previous_decoded_header.insert(header);
                                    break 'decode;
                                }
                                Err(err) => {
                                    trace!("closing session due to err: {err}");
                                    break 'pool;
                                }
                            }
                        }
                    }
                    Err(err) => {
                        panic!("pool.acquire: {}", err);
                    }
                }
            }
        })
    }
}

pub struct Sender<P, S> where P: Pool, S: FnMut(P) -> Packet<<P::Buffer as Owned>::Shared>{
    stream: std::net::TcpStream,

}

impl Sender {
    fn new(stream: std::net::TcpStream) -> Self {
        Self { stream }
    }

    pub fn run(self) -> JoinHandle<()> {
        let Self { stream } = self;

        std::thread::spawn(move || loop {})
    }
}
