use renet::{
    channel::reliable::ReliableChannelConfig,
    error::ClientNotFound,
    packet::Payload,
    remote_connection::ConnectionConfig,
    server::{ConnectionPermission, SendTarget, Server, ServerConfig, ServerEvent},
};

use std::net::{SocketAddr, UdpSocket};
use log::error;
use crate::RenetUdpError;

// TODO: use macro delegate!
pub struct UdpServer {
    socket: UdpSocket,
    server: Server<SocketAddr>,
    buffer: Vec<u8>,
}

impl UdpServer {
    pub fn new(
        config: ServerConfig,
        connection_config: ConnectionConfig,
        connection_permission: ConnectionPermission,
        reliable_channels_config: Vec<ReliableChannelConfig>,
        socket: UdpSocket,
    ) -> Result<Self, std::io::Error> {
        let buffer = vec![0u8; connection_config.max_packet_size as usize];
        let server = Server::new(
            config,
            connection_config,
            connection_permission,
            reliable_channels_config,
        );
        socket.set_nonblocking(true)?;

        Ok(Self {
            socket,
            server,
            buffer,
        })
    }

    pub fn addr(&self) -> Result<SocketAddr, std::io::Error> {
        self.socket.local_addr()
    }

    pub fn get_event(&mut self) -> Option<ServerEvent<SocketAddr>> {
        self.server.get_event()
    }

    pub fn disconnect(&mut self, client_id: &SocketAddr) {
        if let Ok(Some(packet)) = self.server.disconnect(client_id) {
            if let Err(e) = self.socket.send_to(&packet, client_id) {
                error!("failed to send disconnect packet to {}: {}", client_id, e);
            }
        }
    }

    pub fn set_connection_permission(&mut self, connection_permission: ConnectionPermission) {
        self.server.set_connection_permission(connection_permission);
    }

    pub fn deny_client(&mut self, client_id: &SocketAddr) {
        self.server.deny_client(client_id);
    }

    pub fn allow_client(&mut self, client_id: &SocketAddr) {
        self.server.allow_client(client_id);
    }

    pub fn allowed_clients(&self) -> Vec<SocketAddr> {
        self.server.allowed_clients()
    }

    pub fn denied_clients(&self) -> Vec<SocketAddr> {
        self.server.denied_clients()
    }

    pub fn update(&mut self) {
        loop {
            match self.socket.recv_from(&mut self.buffer) {
                Ok((len, addr)) => {
                    if !self.server.is_client_connected(&addr) {
                        if let Err(reason) = self.server.add_connection(&addr) {
                            if let Ok(packet) = reason.as_packet() {
                                if let Err(e) = self.socket.send_to(&packet, addr) {
                                    error!("failed to send disconnect packet to {}: {}", addr, e);
                                }
                            }
                        }
                    }
                    match self.server.process_payload_from(&self.buffer[..len], addr) {
                        Err(_) => {}
                        Ok(()) => {}
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(_) => return, //Err(e),
            };
        }

        self.server.verify_disconnections();
    }

    pub fn receive_reliable_message(
        &mut self,
        client: &SocketAddr,
        channel_id: u8,
    ) -> Result<Option<Payload>, ClientNotFound> {
        self.server.receive_reliable_message(client, channel_id)
    }

    pub fn receive_unreliable_message(
        &mut self,
        client: &SocketAddr,
    ) -> Result<Option<Payload>, ClientNotFound> {
        self.server.receive_unreliable_message(client)
    }

    pub fn receive_block_message(
        &mut self,
        client: &SocketAddr,
    ) -> Result<Option<Payload>, ClientNotFound> {
        self.server.receive_unreliable_message(client)
    }

    pub fn send_reliable_message<ChannelId: Into<u8>>(
        &mut self,
        send_target: SendTarget<SocketAddr>,
        channel_id: ChannelId,
        message: Vec<u8>,
    ) {
        self.server
            .send_reliable_message(send_target, channel_id, message)
    }

    pub fn send_unreliable_message(
        &mut self,
        send_target: SendTarget<SocketAddr>,
        message: Vec<u8>,
    ) {
        self.server.send_unreliable_message(send_target, message)
    }

    pub fn send_block_message(&mut self, send_target: SendTarget<SocketAddr>, message: Vec<u8>) {
        self.server.send_block_message(send_target, message)
    }

    pub fn send_packets(&mut self) -> Result<(), RenetUdpError> {
        let packets = self.server.get_packets_to_send()?;
        for (addr, packet) in packets.iter() {
            self.socket.send_to(packet, addr)?;
        }

        Ok(())
    }

    pub fn get_clients_id(&self) -> Vec<SocketAddr> {
        self.server.get_clients_id()
    }
}
