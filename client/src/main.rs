use anyhow::bail;
use log::{error, info};
use md5::{Digest, Md5};
use rand::{thread_rng, RngCore};
use std::{
    env::args,
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
};

#[derive(Default, Clone, Debug)]
struct Stats {
    total: u64,
    succeed: u64,
}

fn main() {
    env_logger::init();

    let server_addr: SocketAddr = {
        let server_addr_s = args().nth(1).expect("SERVER ADDRESS MUST BE SPECIFIED");
        server_addr_s.parse().expect("SERVER ADDRESS MUST BE VALID")
    };
    let concurrency: usize = {
        let concurrency_s = args().nth(2).expect("CONCURRENCY MUST BE SPECIFIED");
        concurrency_s.parse().expect("CONCURRENCY MUST BE NUMBER")
    };
    let stats = Arc::new(Mutex::new(Stats::default()));
    let interrupted = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&interrupted))
        .expect("FAILED TO SETUP SIGNAL HANDLER");

    let mut threads = Vec::new();
    for _ in 0..concurrency {
        let server_addr = server_addr.to_owned();
        let stats = stats.to_owned();
        let interrupted = interrupted.to_owned();
        threads.push(thread::spawn(move || {
            thread_worker(server_addr, stats, interrupted)
        }));
    }
    for thread in threads {
        thread.join().expect("THREAD JOIN FAILED");
    }
}

fn thread_worker(server_addr: SocketAddr, stats: Arc<Mutex<Stats>>, interrupted: Arc<AtomicBool>) {
    if let Err(err) = _thread_worker(server_addr, stats, interrupted) {
        error!("Start thread worker error: {}", err);
    }

    fn _thread_worker(
        server_addr: SocketAddr,
        stats: Arc<Mutex<Stats>>,
        interrupted: Arc<AtomicBool>,
    ) -> anyhow::Result<()> {
        let mut udp_socket = UdpSocket::bind((IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0))?;
        udp_socket.connect(server_addr)?;

        while !interrupted.load(Ordering::SeqCst) {
            stats.lock().unwrap().total += 1;
            if let Err(err) = _echo_client(&mut udp_socket) {
                let stats = stats.lock().unwrap();
                error!("Echo error: {} ({} / {})", err, stats.succeed, stats.total);
            } else {
                let mut stats = stats.lock().unwrap();
                stats.succeed += 1;
                info!("Echo ok: ({} / {})", stats.succeed, stats.total);
            }
        }

        Ok(())
    }

    fn _echo_client(udp_socket: &mut UdpSocket) -> anyhow::Result<()> {
        const PACKET_SIZE: usize = 1200;
        const CHECKSUM_SIZE: usize = 16;

        let mut rng = thread_rng();
        let mut hasher = Md5::new();

        let mut send_buf = [0u8; PACKET_SIZE];
        let mut recv_buf = [0u8; PACKET_SIZE];

        {
            let mut content = &mut send_buf[..(PACKET_SIZE - CHECKSUM_SIZE)];
            rng.fill_bytes(&mut content);
            hasher.update(&content);
        }

        {
            let checksum = &mut send_buf[(PACKET_SIZE - CHECKSUM_SIZE)..PACKET_SIZE];
            checksum.copy_from_slice(hasher.finalize().as_slice());
        }

        udp_socket.send(&send_buf)?;
        let received = udp_socket.recv(&mut recv_buf)?;

        if received != send_buf.len() {
            bail!("Short received: {}", received);
        } else if recv_buf[..received] != send_buf {
            bail!("Invalid packet");
        }

        Ok(())
    }
}
