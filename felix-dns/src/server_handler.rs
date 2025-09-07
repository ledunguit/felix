use std::{net::SocketAddr, sync::Arc, time::Duration};

use anyhow::{Context, Result};
use tokio::{net::UdpSocket, sync::oneshot, time::timeout};
use trust_dns_proto::{
    op::{Message, MessageType, OpCode},
    rr::{Name, RData, Record, RecordType},
    serialize::binary::{BinEncodable, BinEncoder},
};

use crate::ResolverState;

pub struct ServerHandle {
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl ServerHandle {
    pub async fn shutdown(mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

pub async fn run_udp_server(listen_addr: SocketAddr, state: ResolverState) -> Result<ServerHandle> {
    let socket = UdpSocket::bind(listen_addr)
        .await
        .with_context(|| format!("binding udp socket to {}", listen_addr))?;

    log::info!("Local DNS UDP listening on {}", listen_addr);

    let (shutdown_tx, mut shutdown_rx) = oneshot::channel();

    let socket = Arc::new(socket);
    let state_clone = state.clone();

    let s = socket.clone();

    tokio::spawn(async move {
        let mut buf = vec![0u8; 2048];
        loop {
            tokio::select! {
                biased;
                _ = &mut shutdown_rx => {
                    log::info!("Shutting down DNS server");
                    break;
                }
                recv = s.recv_from(&mut buf) => {
                    match recv {
                        Ok((n, peer)) => {
                            let packet = buf[..n].to_vec();
                            let st = state_clone.clone();
                            let s2 = s.clone();
                            // spawn to handle concurrently
                            tokio::spawn(async move {
                                if let Err(e) = handle_packet(packet, peer, s2, st).await {
                                    log::warn!("Error handling DNS packet from {}: {:?}", peer, e);
                                }
                            });
                        }
                        Err(e) => {
                            log::warn!("recv_from error: {:?}", e);
                        }
                    }
                }
            }
        }
    });

    Ok(ServerHandle {
        shutdown_tx: Some(shutdown_tx),
    })
}

async fn handle_packet(
    packet: Vec<u8>,
    src: SocketAddr,
    socket: Arc<UdpSocket>,
    state: ResolverState,
) -> anyhow::Result<()> {
    // parse message
    let msg = match Message::from_vec(&packet) {
        Ok(m) => m,
        Err(e) => {
            log::warn!("Failed to parse DNS message from {}: {:?}", src, e);
            return Ok(());
        }
    };

    // we handle only first query
    if msg.queries().is_empty() {
        // ignore
        return Ok(());
    }
    let query = &msg.queries()[0];
    let qname = query.name().to_utf8();
    let qtype = query.query_type();

    log::debug!("Query from {}: {} {:?}", src, qname, qtype);

    // try local resolve if enabled and mapping exists (only A)
    if let Ok(Some(ip)) = state.resolve(&qname).await {
        // Only answer A queries or ANY
        if qtype == RecordType::A || qtype == RecordType::ANY {
            let mut resp = Message::new();
            resp.set_id(msg.id());
            resp.set_message_type(MessageType::Response);
            resp.set_op_code(OpCode::Query);
            resp.set_authoritative(true);
            resp.add_query(query.clone());

            let name = Name::from_utf8(&qname)?;
            let record = Record::from_rdata(name, 60, RData::A(ip.into()));
            resp.add_answer(record);

            let mut out: Vec<u8> = Vec::with_capacity(512);
            {
                let mut encoder = BinEncoder::new(&mut out);
                resp.emit(&mut encoder)?;
            }
            socket.send_to(&out, src).await?;
            log::info!("Answered {} -> {} to {}", qname, ip, src);
            return Ok(());
        }
    }

    let upstream = state.upstream();
    match forward_udp_and_relay(&packet, upstream, &socket, src).await {
        Ok(_) => Ok(()),
        Err(e) => {
            log::warn!("Forwarding failed: {:?}", e);

            // Create response with SERVFAIL
            let mut resp = Message::new();
            resp.set_id(msg.id());
            resp.set_message_type(MessageType::Response);
            resp.set_op_code(OpCode::Query);
            resp.set_authoritative(true);
            resp.set_response_code(trust_dns_proto::op::ResponseCode::ServFail);
            resp.add_query(query.clone());

            let mut out: Vec<u8> = Vec::with_capacity(512);
            {
                let mut encoder = BinEncoder::new(&mut out);
                resp.emit(&mut encoder)?;
            }
            socket.send_to(&out, src).await?;

            log::info!("Answered {} -> SERVFAIL to {}", qname, src);

            Err(e)
        }
    }
}

async fn forward_udp_and_relay(
    packet: &[u8],
    upstream: SocketAddr,
    socket: &UdpSocket,
    client: SocketAddr,
) -> anyhow::Result<()> {
    // talk to upstream using ephemeral socket
    let upstream_socket = UdpSocket::bind("0.0.0.0:0").await?;
    upstream_socket.send_to(packet, upstream).await?;

    // wait for response with timeout
    let mut buf = vec![0u8; 4096];
    let n = timeout(Duration::from_secs(2), upstream_socket.recv_from(&mut buf)).await??;
    let (size, _peer) = n;
    socket.send_to(&buf[..size], client).await?;
    println!("Forwarding to {} from {}", client, upstream);
    Ok(())
}
