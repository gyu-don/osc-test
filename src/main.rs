use std::convert::TryFrom;
use std::env::args;
use std::net::SocketAddr;
use std::str::FromStr;

use tokio::task;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;

use anyhow::{anyhow, bail, ensure};

#[allow(unused_imports)]
use log::{LevelFilter, info, warn};

use message::{Request, Response};
use rosc::{OscBundle, OscMessage, OscPacket};

pub mod message;

const HOST_QUEUE_LEN: usize = 100;
const DEVICE_QUEUE_LEN: usize = 100;
const OSC_BUF_LEN: usize = 1000;

async fn device_sender_loop(tx_addr: SocketAddr, mut chan_rx: mpsc::Receiver<Request>) -> anyhow::Result<()> {
    let tx = UdpSocket::bind(tx_addr).await?;
    while let Some(msg) = chan_rx.recv().await {
        info!("device_sender_loop: Received from channel: {:?}", msg);
        let packet = rosc::encoder::encode(&OscPacket::Bundle(
                OscBundle { timetag: (0, 0),
                            content: vec![OscPacket::Message(OscMessage::from(&msg))]
                })).map_err(|e| anyhow!("{:?}", e))?;
        tx.send(&packet).await?;
    }
    bail!("device_sender_loop unexpected finished");
}

async fn device_receiver_loop(rx_addr: SocketAddr, chan_tx: mpsc::Sender<Response>) -> anyhow::Result<()> {
    let mut buf = vec![0u8; OSC_BUF_LEN];
    let rx = UdpSocket::bind(rx_addr).await?;
    loop {
        let len = rx.recv(&mut buf).await?;
        let packet = rosc::decoder::decode(&buf[..len]).map_err(|e| anyhow!("{:?}", e))?;
        let msg = Response::try_from(match packet {
            OscPacket::Message(msg) => {
                warn!("Message without Bundle");
                msg
            },
            OscPacket::Bundle(mut bundle) => {
                ensure!(bundle.content.len() != 0, "Received empty bundle.");
                ensure!(bundle.content.len() == 1, "Multiple messages in same bundle.");
                match bundle.content.pop().unwrap() {
                    OscPacket::Message(msg) => msg,
                    OscPacket::Bundle(_bundle) => bail!("Received nested bundle.")
                }
            }
        })?;
        info!("Received from device: {:?}", msg);
        chan_tx.send(msg).await?;
        buf.clear();
    }
}

async fn host_sender_loop(tx_addr: SocketAddr, mut chan_rx: mpsc::Receiver<Response>) -> anyhow::Result<()> {
    let tx = UdpSocket::bind(tx_addr).await?;
    while let Some(msg) = chan_rx.recv().await {
        info!("host_sender_loop: Received from channel: {:?}", msg);
        let packet = rosc::encoder::encode(&OscPacket::Bundle(
                OscBundle { timetag: (0, 0),
                            content: vec![OscPacket::Message(OscMessage::from(&msg))]
                })).map_err(|e| anyhow!("{:?}", e))?;
        tx.send(&packet).await?;
    }
    bail!("host_sender_loop unexpected finished");
}

async fn host_receiver_loop(host_rx_addr: SocketAddr, chan_tx: mpsc::Sender<Request>) -> anyhow::Result<()> {
    let rx = UdpSocket::bind(host_rx_addr).await?;
    let mut buf = vec![0; OSC_BUF_LEN];
    loop {
        info!("Receiving from {}...", host_rx_addr);
        let len = rx.recv(&mut buf).await?;
        let packet = rosc::decoder::decode(&buf[..len]).map_err(|e| anyhow!("{:?}", e))?;
        let msg = Request::try_from(match packet {
            OscPacket::Message(msg) => {
                warn!("Message without Bundle");
                msg
            },
            OscPacket::Bundle(mut bundle) => {
                ensure!(bundle.content.len() != 0, "Received empty bundle.");
                ensure!(bundle.content.len() == 1, "Multiple messages in same bundle.");
                match bundle.content.pop().unwrap() {
                    OscPacket::Message(msg) => msg,
                    OscPacket::Bundle(_bundle) => bail!("Received nested bundle.")
                }
            }
        })?;
        buf.clear();
        chan_tx.send(msg).await?;
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_default_env().filter_level(LevelFilter::Info).init();
    let host_tx = args().nth(1)
                        .and_then(|s| SocketAddr::from_str(&s).ok())
                        .expect("Sender addr required.");
    let host_rx = args().nth(2)
                        .and_then(|s| SocketAddr::from_str(&s).ok())
                        .expect("Receiver addr required.");
    let device_tx = args().nth(3)
                          .and_then(|s| SocketAddr::from_str(&s).ok())
                          .expect("Sender addr required.");
    let device_rx = args().nth(4)
                          .and_then(|s| SocketAddr::from_str(&s).ok())
                          .expect("Receiver addr required.");
    let (chan_tx, chan_rx) = mpsc::channel(HOST_QUEUE_LEN);
    let (chan2_tx, chan2_rx) = mpsc::channel(DEVICE_QUEUE_LEN);
    let host_receiver = task::spawn(host_receiver_loop(host_rx, chan_tx));
    let device_sender = task::spawn(device_sender_loop(device_tx, chan_rx));
    let device_receiver = task::spawn(device_receiver_loop(device_rx, chan2_tx));
    let host_sender = task::spawn(host_sender_loop(host_tx, chan2_rx));

    host_receiver.await??;
    device_sender.await??;
    device_receiver.await??;
    host_sender.await??;
    Ok(())
}
