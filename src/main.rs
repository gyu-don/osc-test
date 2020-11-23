use std::collections::VecDeque;
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

use message::Message;
use rosc::{OscBundle, OscMessage, OscPacket};

pub mod message;

const HOST_QUEUE_LEN: usize = 100;

async fn device_sender_loop(tx_addr: SocketAddr, mut chan_rx: mpsc::Receiver<Message>) -> anyhow::Result<()> {
    let tx = UdpSocket::bind(tx_addr).await?;
    while let Some(msg) = chan_rx.recv().await {
        info!("Received: {:?}", msg);
        let packet = rosc::encoder::encode(&OscPacket::Bundle(
                OscBundle { timetag: (0, 0),
                            content: vec![OscPacket::Message(OscMessage::from(&msg))]
                })).map_err(|e| anyhow!("{:?}", e))?;
        tx.send(&packet).await?;
    }
    bail!("host_sender_loop unexpected finished");
}

async fn host_receiver_loop(tx_addr: SocketAddr, rx_addr: SocketAddr, chan_tx: mpsc::Sender<Message>) -> anyhow::Result<()> {
    let tx = UdpSocket::bind(tx_addr).await?;
    let rx = UdpSocket::bind(rx_addr).await?;
    let mut buf = vec![0; 1000];
    loop {
        info!("Receiving from {}...", rx_addr);
        let len = rx.recv(&mut buf).await?;
        let packet = rosc::decoder::decode(&buf[..len]).map_err(|e| anyhow!("{:?}", e))?;
        let msg = Message::try_from(match packet {
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
        let meas_fut = async {
        };
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
    // info!("{:?}, {:?}", host_tx, host_rx);
    // info!("{:?}, {:?}", device_tx, device_rx);
    let host_receiver = task::spawn(host_receiver_loop(host_tx, host_rx, chan_tx));
    let device_sender = task::spawn(device_sender_loop(device_tx, chan_rx));

    host_receiver.await??;
    device_sender.await??;
    Ok(())
}
