use std::env::args;
use std::net::{SocketAddr, UdpSocket};
use std::str::FromStr;

pub mod message;

fn main() -> anyhow::Result<()> {
    let host_tx = args().nth(1)
                        .and_then(|s| SocketAddr::from_str(&s).ok())
                        .expect("Sender addr required.");
    let host_tx = UdpSocket::bind(host_tx)?;
    let host_rx = args().nth(2)
                        .and_then(|s| SocketAddr::from_str(&s).ok())
                        .expect("Receiver addr required.");
    let host_rx = UdpSocket::bind(host_rx)?;
    println!("{:?}, {:?}", host_tx, host_rx);
    let device_tx = args().nth(3)
                          .and_then(|s| SocketAddr::from_str(&s).ok())
                          .expect("Sender addr required.");
    let device_tx = UdpSocket::bind(device_tx)?;
    let device_rx = args().nth(4)
                          .and_then(|s| SocketAddr::from_str(&s).ok())
                          .expect("Receiver addr required.");
    let device_rx = UdpSocket::bind(device_rx)?;
    println!("{:?}, {:?}", device_tx, device_rx);
    Ok(())
}
