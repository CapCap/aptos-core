// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

//! Integration tests for validator_network.

use crate::builder::NetworkBuilder;
use aptos_config::{
    config::{Peer, PeerRole, PeerSet, RoleType, NETWORK_CHANNEL_SIZE},
    network_id::{NetworkContext, NetworkId},
};
use aptos_crypto::{test_utils::TEST_SEED, x25519, Uniform};
use aptos_infallible::RwLock;
use aptos_time_service::TimeService;
use aptos_types::{chain_id::ChainId, network_address::NetworkAddress, PeerId};
use async_trait::async_trait;
use channel::aptos_channel;
use futures::{executor::block_on, StreamExt};
use netcore::transport::ConnectionOrigin;
use network::{
    application::storage::PeerMetadataStorage,
    error::NetworkError,
    peer_manager::{
        builder::AuthenticationMode, ConnectionRequestSender, PeerManagerRequestSender,
    },
    protocols::{
        network::{
            AppConfig, ApplicationNetworkSender, Event, NetworkEvents, NetworkSender,
            NewNetworkSender,
        },
        rpc::error::RpcError,
    },
    ProtocolId,
};
use rand::{rngs::StdRng, SeedableRng};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};
use tokio::runtime::Runtime;

const TEST_RPC_PROTOCOL: ProtocolId = ProtocolId::ConsensusRpcBcs;
const TEST_DIRECT_SEND_PROTOCOL: ProtocolId = ProtocolId::ConsensusDirectSendBcs;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DummyMsg(pub Vec<u8>);

pub fn network_endpoint_config() -> AppConfig {
    AppConfig::p2p(
        [TEST_RPC_PROTOCOL, TEST_DIRECT_SEND_PROTOCOL],
        aptos_channel::Config::new(NETWORK_CHANNEL_SIZE),
    )
}

/// TODO(davidiw): In DummyNetwork, replace DummyMsg with a Serde compatible type once migration
/// is complete
pub type DummyNetworkEvents = NetworkEvents<DummyMsg>;

#[derive(Clone)]
pub struct DummyNetworkSender {
    inner: NetworkSender<DummyMsg>,
}

impl NewNetworkSender for DummyNetworkSender {
    fn new(
        peer_mgr_reqs_tx: PeerManagerRequestSender,
        connection_reqs_tx: ConnectionRequestSender,
    ) -> Self {
        Self {
            inner: NetworkSender::new(peer_mgr_reqs_tx, connection_reqs_tx),
        }
    }
}

#[async_trait]
impl ApplicationNetworkSender<DummyMsg> for DummyNetworkSender {
    fn send_to(&self, recipient: PeerId, message: DummyMsg) -> Result<(), NetworkError> {
        let protocol = TEST_DIRECT_SEND_PROTOCOL;
        self.inner.send_to(recipient, protocol, message)
    }

    async fn send_rpc(
        &self,
        recipient: PeerId,
        message: DummyMsg,
        timeout: Duration,
    ) -> Result<DummyMsg, RpcError> {
        let protocol = TEST_RPC_PROTOCOL;
        self.inner
            .send_rpc(recipient, protocol, message, timeout)
            .await
    }
}

pub struct DummyNetwork {
    pub runtime: Runtime,
    pub dialer_peer_id: PeerId,
    pub dialer_events: DummyNetworkEvents,
    pub dialer_sender: DummyNetworkSender,
    pub listener_peer_id: PeerId,
    pub listener_events: DummyNetworkEvents,
    pub listener_sender: DummyNetworkSender,
}

/// The following sets up a 2 peer network and verifies connectivity.
pub fn setup_network() -> DummyNetwork {
    let runtime = Runtime::new().unwrap();
    let role = RoleType::Validator;
    let network_id = NetworkId::Validator;
    let chain_id = ChainId::default();
    let dialer_peer_id = PeerId::random();
    let listener_peer_id = PeerId::random();

    // Setup keys for dialer.
    let mut rng = StdRng::from_seed(TEST_SEED);
    let dialer_identity_private_key = x25519::PrivateKey::generate(&mut rng);
    let dialer_identity_public_key = dialer_identity_private_key.public_key();
    let dialer_pubkeys: HashSet<_> = vec![dialer_identity_public_key].into_iter().collect();

    // Setup keys for listener.
    let listener_identity_private_key = x25519::PrivateKey::generate(&mut rng);

    // Setup listen addresses
    let dialer_addr: NetworkAddress = "/ip4/127.0.0.1/tcp/0".parse().unwrap();
    let listener_addr: NetworkAddress = "/ip4/127.0.0.1/tcp/0".parse().unwrap();

    // Setup seed peers
    let mut seeds = PeerSet::new();
    seeds.insert(
        dialer_peer_id,
        Peer::new(vec![], dialer_pubkeys, PeerRole::Validator),
    );

    let trusted_peers = Arc::new(RwLock::new(HashMap::new()));
    let authentication_mode = AuthenticationMode::Mutual(listener_identity_private_key);
    let peer_metadata_storage = PeerMetadataStorage::new(&[NetworkId::Validator]);
    // Set up the listener network
    let network_context = NetworkContext::new(role, network_id, listener_peer_id);
    let mut network_builder = NetworkBuilder::new_for_test(
        chain_id,
        seeds.clone(),
        trusted_peers,
        network_context,
        TimeService::real(),
        listener_addr,
        authentication_mode,
        peer_metadata_storage,
    );

    let (listener_sender, mut listener_events) = network_builder
        .add_p2p_service::<DummyNetworkSender, DummyNetworkEvents>(&network_endpoint_config());
    network_builder.build(runtime.handle().clone()).start();

    // Add the listener address with port
    let listener_addr = network_builder.listen_address();
    seeds.insert(
        listener_peer_id,
        Peer::from_addrs(PeerRole::Validator, vec![listener_addr]),
    );

    let authentication_mode = AuthenticationMode::Mutual(dialer_identity_private_key);

    // Set up the dialer network
    let network_context = NetworkContext::new(role, network_id, dialer_peer_id);

    let trusted_peers = Arc::new(RwLock::new(HashMap::new()));
    let peer_metadata_storage = PeerMetadataStorage::new(&[NetworkId::Validator]);

    let mut network_builder = NetworkBuilder::new_for_test(
        chain_id,
        seeds,
        trusted_peers,
        network_context,
        TimeService::real(),
        dialer_addr,
        authentication_mode,
        peer_metadata_storage,
    );

    let (dialer_sender, mut dialer_events) = network_builder
        .add_p2p_service::<DummyNetworkSender, DummyNetworkEvents>(&network_endpoint_config());
    network_builder.build(runtime.handle().clone()).start();

    // Wait for establishing connection
    let first_dialer_event = block_on(dialer_events.next()).unwrap();
    if let Event::NewPeer(metadata) = first_dialer_event {
        assert_eq!(metadata.remote_peer_id, listener_peer_id);
        assert_eq!(metadata.origin, ConnectionOrigin::Outbound);
        assert_eq!(metadata.role, PeerRole::Validator);
    } else {
        panic!(
            "No NewPeer event on dialer received instead: {:?}",
            first_dialer_event
        );
    }

    let first_listener_event = block_on(listener_events.next()).unwrap();
    if let Event::NewPeer(metadata) = first_listener_event {
        assert_eq!(metadata.remote_peer_id, dialer_peer_id);
        assert_eq!(metadata.origin, ConnectionOrigin::Inbound);
        assert_eq!(metadata.role, PeerRole::Validator);
    } else {
        panic!(
            "No NewPeer event on listener received instead: {:?}",
            first_listener_event
        );
    }

    DummyNetwork {
        runtime,
        dialer_peer_id,
        dialer_events,
        dialer_sender,
        listener_peer_id,
        listener_events,
        listener_sender,
    }
}
