use anyhow::{bail, Context};
use log::{error, info};
use md5::{Digest, Md5};
use std::{
    env::args,
    io::Result as IOResult,
    net::{IpAddr, Ipv4Addr, UdpSocket},
};

fn main() -> IOResult<()> {
    env_logger::init();

    let port: u16 = {
        let port_s = args().nth(1).expect("PORT MUST BE SPECIFIED");
        port_s.parse().expect("PORT MUST BE NUMBER")
    };
    let mut socket = UdpSocket::bind((IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), port))?;

    let mut total = 0;
    let mut succeed = 0;
    loop {
        total += 1;
        echo_server(&mut socket, total, &mut succeed);
    }
}

fn echo_server(socket: &mut UdpSocket, total: u64, succeed: &mut u64) {
    if let Err(err) = _echo_server(socket) {
        error!("Echo error: {} ({} / {})", err, *succeed, total);
    } else {
        *succeed = *succeed + 1;
        info!("Echo ok: ({} / {})", *succeed, total);
    }

    fn _echo_server(socket: &mut UdpSocket) -> anyhow::Result<()> {
        const PACKET_SIZE: usize = 1200;
        const CHECKSUM_SIZE: usize = 16;

        let mut buf = [0u8; PACKET_SIZE];
        let (received, src) = socket
            .recv_from(&mut buf)
            .with_context(|| "Receive error")?;
        if received < CHECKSUM_SIZE {
            bail!("Short received: {}", received);
        }
        let content = &buf[..(received - CHECKSUM_SIZE)];
        let checksum = &buf[(received - CHECKSUM_SIZE)..received];
        let expected = {
            let mut hasher = Md5::new();
            hasher.update(content);
            hasher.finalize()
        };
        if checksum != expected.as_slice() {
            bail!("Invalid checksum");
        }

        socket
            .send_to(&buf[..received], src)
            .with_context(|| "Send error")?;
        Ok(())
    }
}
