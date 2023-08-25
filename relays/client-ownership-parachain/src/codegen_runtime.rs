#![cfg_attr(rustfmt, rustfmt_skip)]
#[allow(dead_code, unused_imports, non_camel_case_types)]
#[allow(clippy::all)]
pub mod api {
        use super::api as root_mod;
        pub mod runtime_types {
                use super::runtime_types;
                pub mod bounded_collections {
                        use super::runtime_types;
                        pub mod bounded_vec {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct BoundedVec<_0>(pub ::std::vec::Vec<_0>);
                        }
                        pub mod weak_bounded_vec {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct WeakBoundedVec<_0>(pub ::std::vec::Vec<_0>);
                        }
                }
                pub mod ep_bridge {
                        use super::runtime_types;
                        pub mod evo_hash {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct EvoHash(pub [::core::primitive::u8; 64usize]);
                        }
                }
                pub mod bp_header_chain {
                        use super::runtime_types;
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub enum HeaderChainError {
                                #[codec(index = 0)]
                                UnknownHeader,
                                #[codec(index = 1)]
                                StorageProof(runtime_types::bp_runtime::storage_proof::StorageProofError),
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub struct StoredHeaderData<_0, _1> {
                                pub number: _0,
                                pub state_root: _1,
                        }
                }
                pub mod bp_messages {
                        use super::runtime_types;
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub struct DeliveredMessages {
                                pub begin: ::core::primitive::u64,
                                pub end: ::core::primitive::u64,
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub struct InboundLaneData<_0> {
                                pub state: runtime_types::bp_messages::LaneState,
                                pub relayers: ::std::vec::Vec<runtime_types::bp_messages::UnrewardedRelayer<_0>>,
                                pub last_confirmed_nonce: ::core::primitive::u64,
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub struct LaneId(pub ::subxt::utils::H256);
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub enum LaneState {
                                #[codec(index = 0)]
                                Opened,
                                #[codec(index = 1)]
                                Closed,
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub struct MessageKey {
                                pub lane_id: runtime_types::bp_messages::LaneId,
                                pub nonce: ::core::primitive::u64,
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub enum MessagesOperatingMode {
                                #[codec(index = 0)]
                                Basic(runtime_types::bp_runtime::BasicOperatingMode),
                                #[codec(index = 1)]
                                RejectingOutboundMessages,
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub struct OutboundLaneData {
                                pub state: runtime_types::bp_messages::LaneState,
                                pub oldest_unpruned_nonce: ::core::primitive::u64,
                                pub latest_received_nonce: ::core::primitive::u64,
                                pub latest_generated_nonce: ::core::primitive::u64,
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub enum ReceivalResult<_0> {
                                #[codec(index = 0)]
                                Dispatched(runtime_types::bp_runtime::messages::MessageDispatchResult<_0>),
                                #[codec(index = 1)]
                                InvalidNonce,
                                #[codec(index = 2)]
                                TooManyUnrewardedRelayers,
                                #[codec(index = 3)]
                                TooManyUnconfirmedMessages,
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub struct ReceivedMessages<_0> {
                                pub lane: runtime_types::bp_messages::LaneId,
                                pub receive_results: ::std::vec::Vec<(
                                        ::core::primitive::u64,
                                        runtime_types::bp_messages::ReceivalResult<_0>,
                                )>,
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub struct UnrewardedRelayer<_0> {
                                pub relayer: _0,
                                pub messages: runtime_types::bp_messages::DeliveredMessages,
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub enum VerificationError {
                                #[codec(index = 0)]
                                EmptyMessageProof,
                                #[codec(index = 1)]
                                HeaderChain(runtime_types::bp_header_chain::HeaderChainError),
                                #[codec(index = 2)]
                                InboundLaneStorage(runtime_types::bp_runtime::storage_proof::StorageProofError),
                                #[codec(index = 3)]
                                InvalidMessageWeight,
                                #[codec(index = 4)]
                                MessagesCountMismatch,
                                #[codec(index = 5)]
                                MessageStorage(runtime_types::bp_runtime::storage_proof::StorageProofError),
                                #[codec(index = 6)]
                                MessageTooLarge,
                                #[codec(index = 7)]
                                OutboundLaneStorage(runtime_types::bp_runtime::storage_proof::StorageProofError),
                                #[codec(index = 8)]
                                StorageProof(runtime_types::bp_runtime::storage_proof::StorageProofError),
                                #[codec(index = 9)]
                                Other,
                        }
                }
                pub mod bp_relayers {
                        use super::runtime_types;
                        pub mod registration {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct Registration<_0, _1> {
                                        pub valid_till: _0,
                                        pub stake: _1,
                                }
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub enum RewardsAccountOwner {
                                #[codec(index = 0)]
                                ThisChain,
                                #[codec(index = 1)]
                                BridgedChain,
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub struct RewardsAccountParams {
                                pub owner: runtime_types::bp_relayers::RewardsAccountOwner,
                                pub bridged_chain_id: [::core::primitive::u8; 4usize],
                                pub lane_id: runtime_types::bp_messages::LaneId,
                        }
                }
                pub mod bp_runtime {
                        use super::runtime_types;
                        pub mod messages {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct MessageDispatchResult<_0> {
                                        pub unspent_weight: ::sp_weights::Weight,
                                        pub dispatch_level_result: _0,
                                }
                        }
                        pub mod storage_proof {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum StorageProofError {
                                        #[codec(index = 0)]
                                        UnableToGenerateTrieProof,
                                        #[codec(index = 1)]
                                        InvalidProof,
                                        #[codec(index = 2)]
                                        UnsortedEntries,
                                        #[codec(index = 3)]
                                        UnavailableKey,
                                        #[codec(index = 4)]
                                        EmptyVal,
                                        #[codec(index = 5)]
                                        DecodeError,
                                        #[codec(index = 6)]
                                        UnusedKey,
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct UnverifiedStorageProof {
                                        pub proof: ::std::vec::Vec<::std::vec::Vec<::core::primitive::u8>>,
                                        pub db: ::std::vec::Vec<(
                                                ::std::vec::Vec<::core::primitive::u8>,
                                                ::core::option::Option<::std::vec::Vec<::core::primitive::u8>>,
                                        )>,
                                }
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub enum BasicOperatingMode {
                                #[codec(index = 0)]
                                Normal,
                                #[codec(index = 1)]
                                Halted,
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub struct HeaderId<_0, _1>(pub _1, pub _0);
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub enum OwnedBridgeModuleError {
                                #[codec(index = 0)]
                                Halted,
                        }
                }
                pub mod bridge_runtime_common {
                        use super::runtime_types;
                        pub mod messages_xcm_extension {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum XcmBlobMessageDispatchResult {
                                        #[codec(index = 0)]
                                        InvalidPayload,
                                        #[codec(index = 1)]
                                        Dispatched,
                                        #[codec(index = 2)]
                                        NotDispatched,
                                }
                        }
                }
                pub mod cumulus_pallet_dmp_queue {
                        use super::runtime_types;
                        pub mod pallet {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Call {
                                        #[codec(index = 0)]
                                        service_overweight {
                                                index: ::core::primitive::u64,
                                                weight_limit: ::sp_weights::Weight,
                                        },
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Error {
                                        #[codec(index = 0)]
                                        Unknown,
                                        #[codec(index = 1)]
                                        OverLimit,
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Event {
                                        #[codec(index = 0)]
                                        InvalidFormat { message_hash: [::core::primitive::u8; 32usize] },
                                        #[codec(index = 1)]
                                        UnsupportedVersion { message_hash: [::core::primitive::u8; 32usize] },
                                        #[codec(index = 2)]
                                        ExecutedDownward {
                                                message_hash: [::core::primitive::u8; 32usize],
                                                message_id: [::core::primitive::u8; 32usize],
                                                outcome: runtime_types::xcm::v3::traits::Outcome,
                                        },
                                        #[codec(index = 3)]
                                        WeightExhausted {
                                                message_hash: [::core::primitive::u8; 32usize],
                                                message_id: [::core::primitive::u8; 32usize],
                                                remaining_weight: ::sp_weights::Weight,
                                                required_weight: ::sp_weights::Weight,
                                        },
                                        #[codec(index = 4)]
                                        OverweightEnqueued {
                                                message_hash: [::core::primitive::u8; 32usize],
                                                message_id: [::core::primitive::u8; 32usize],
                                                overweight_index: ::core::primitive::u64,
                                                required_weight: ::sp_weights::Weight,
                                        },
                                        #[codec(index = 5)]
                                        OverweightServiced {
                                                overweight_index: ::core::primitive::u64,
                                                weight_used: ::sp_weights::Weight,
                                        },
                                        #[codec(index = 6)]
                                        MaxMessagesExhausted { message_hash: [::core::primitive::u8; 32usize] },
                                }
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub struct ConfigData {
                                pub max_individual: ::sp_weights::Weight,
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub struct PageIndexData {
                                pub begin_used: ::core::primitive::u32,
                                pub end_used: ::core::primitive::u32,
                                pub overweight_count: ::core::primitive::u64,
                        }
                }
                pub mod cumulus_pallet_parachain_system {
                        use super::runtime_types;
                        pub mod pallet {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Call {
                                        # [codec (index = 0)] set_validation_data { data : runtime_types :: cumulus_primitives_parachain_inherent :: ParachainInherentData , } , # [codec (index = 1)] sudo_send_upward_message { message : :: std :: vec :: Vec < :: core :: primitive :: u8 > , } , # [codec (index = 2)] authorize_upgrade { code_hash : :: subxt :: utils :: H256 , check_version : :: core :: primitive :: bool , } , # [codec (index = 3)] enact_authorized_upgrade { code : :: std :: vec :: Vec < :: core :: primitive :: u8 > , } , }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Error {
                                        #[codec(index = 0)]
                                        OverlappingUpgrades,
                                        #[codec(index = 1)]
                                        ProhibitedByPolkadot,
                                        #[codec(index = 2)]
                                        TooBig,
                                        #[codec(index = 3)]
                                        ValidationDataNotAvailable,
                                        #[codec(index = 4)]
                                        HostConfigurationNotAvailable,
                                        #[codec(index = 5)]
                                        NotScheduled,
                                        #[codec(index = 6)]
                                        NothingAuthorized,
                                        #[codec(index = 7)]
                                        Unauthorized,
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Event {
                                        #[codec(index = 0)]
                                        ValidationFunctionStored,
                                        #[codec(index = 1)]
                                        ValidationFunctionApplied { relay_chain_block_num: ::core::primitive::u32 },
                                        #[codec(index = 2)]
                                        ValidationFunctionDiscarded,
                                        #[codec(index = 3)]
                                        UpgradeAuthorized { code_hash: ::subxt::utils::H256 },
                                        #[codec(index = 4)]
                                        DownwardMessagesReceived { count: ::core::primitive::u32 },
                                        #[codec(index = 5)]
                                        DownwardMessagesProcessed {
                                                weight_used: ::sp_weights::Weight,
                                                dmq_head: ::subxt::utils::H256,
                                        },
                                        #[codec(index = 6)]
                                        UpwardMessageSent {
                                                message_hash: ::core::option::Option<[::core::primitive::u8; 32usize]>,
                                        },
                                }
                        }
                        pub mod relay_state_snapshot {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct MessagingStateSnapshot { pub dmq_mqc_head : :: subxt :: utils :: H256 , pub relay_dispatch_queue_size : runtime_types :: cumulus_pallet_parachain_system :: relay_state_snapshot :: RelayDispachQueueSize , pub ingress_channels : :: std :: vec :: Vec < (runtime_types :: polkadot_parachain :: primitives :: Id , runtime_types :: polkadot_primitives :: v5 :: AbridgedHrmpChannel ,) > , pub egress_channels : :: std :: vec :: Vec < (runtime_types :: polkadot_parachain :: primitives :: Id , runtime_types :: polkadot_primitives :: v5 :: AbridgedHrmpChannel ,) > , }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct RelayDispachQueueSize {
                                        pub remaining_count: ::core::primitive::u32,
                                        pub remaining_size: ::core::primitive::u32,
                                }
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub struct CodeUpgradeAuthorization {
                                pub code_hash: ::subxt::utils::H256,
                                pub check_version: ::core::primitive::bool,
                        }
                }
                pub mod cumulus_pallet_xcm {
                        use super::runtime_types;
                        pub mod pallet {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Call {}
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Error {}
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Event {
                                        #[codec(index = 0)]
                                        InvalidFormat([::core::primitive::u8; 32usize]),
                                        #[codec(index = 1)]
                                        UnsupportedVersion([::core::primitive::u8; 32usize]),
                                        #[codec(index = 2)]
                                        ExecutedDownward(
                                                [::core::primitive::u8; 32usize],
                                                runtime_types::xcm::v3::traits::Outcome,
                                        ),
                                }
                        }
                }
                pub mod cumulus_pallet_xcmp_queue {
                        use super::runtime_types;
                        pub mod pallet {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Call {
                                        #[codec(index = 0)]
                                        service_overweight {
                                                index: ::core::primitive::u64,
                                                weight_limit: ::sp_weights::Weight,
                                        },
                                        #[codec(index = 1)]
                                        suspend_xcm_execution,
                                        #[codec(index = 2)]
                                        resume_xcm_execution,
                                        #[codec(index = 3)]
                                        update_suspend_threshold { new: ::core::primitive::u32 },
                                        #[codec(index = 4)]
                                        update_drop_threshold { new: ::core::primitive::u32 },
                                        #[codec(index = 5)]
                                        update_resume_threshold { new: ::core::primitive::u32 },
                                        #[codec(index = 6)]
                                        update_threshold_weight { new: ::sp_weights::Weight },
                                        #[codec(index = 7)]
                                        update_weight_restrict_decay { new: ::sp_weights::Weight },
                                        #[codec(index = 8)]
                                        update_xcmp_max_individual_weight { new: ::sp_weights::Weight },
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Error {
                                        #[codec(index = 0)]
                                        FailedToSend,
                                        #[codec(index = 1)]
                                        BadXcmOrigin,
                                        #[codec(index = 2)]
                                        BadXcm,
                                        #[codec(index = 3)]
                                        BadOverweightIndex,
                                        #[codec(index = 4)]
                                        WeightOverLimit,
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Event {
                                        #[codec(index = 0)]
                                        Success {
                                                message_hash: [::core::primitive::u8; 32usize],
                                                message_id: [::core::primitive::u8; 32usize],
                                                weight: ::sp_weights::Weight,
                                        },
                                        #[codec(index = 1)]
                                        Fail {
                                                message_hash: [::core::primitive::u8; 32usize],
                                                message_id: [::core::primitive::u8; 32usize],
                                                error: runtime_types::xcm::v3::traits::Error,
                                                weight: ::sp_weights::Weight,
                                        },
                                        #[codec(index = 2)]
                                        BadVersion { message_hash: [::core::primitive::u8; 32usize] },
                                        #[codec(index = 3)]
                                        BadFormat { message_hash: [::core::primitive::u8; 32usize] },
                                        #[codec(index = 4)]
                                        XcmpMessageSent { message_hash: [::core::primitive::u8; 32usize] },
                                        #[codec(index = 5)]
                                        OverweightEnqueued {
                                                sender: runtime_types::polkadot_parachain::primitives::Id,
                                                sent_at: ::core::primitive::u32,
                                                index: ::core::primitive::u64,
                                                required: ::sp_weights::Weight,
                                        },
                                        #[codec(index = 6)]
                                        OverweightServiced { index: ::core::primitive::u64, used: ::sp_weights::Weight },
                                }
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub struct InboundChannelDetails {
                                pub sender: runtime_types::polkadot_parachain::primitives::Id,
                                pub state: runtime_types::cumulus_pallet_xcmp_queue::InboundState,
                                pub message_metadata: ::std::vec::Vec<(
                                        ::core::primitive::u32,
                                        runtime_types::polkadot_parachain::primitives::XcmpMessageFormat,
                                )>,
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub enum InboundState {
                                #[codec(index = 0)]
                                Ok,
                                #[codec(index = 1)]
                                Suspended,
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub struct OutboundChannelDetails {
                                pub recipient: runtime_types::polkadot_parachain::primitives::Id,
                                pub state: runtime_types::cumulus_pallet_xcmp_queue::OutboundState,
                                pub signals_exist: ::core::primitive::bool,
                                pub first_index: ::core::primitive::u16,
                                pub last_index: ::core::primitive::u16,
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub enum OutboundState {
                                #[codec(index = 0)]
                                Ok,
                                #[codec(index = 1)]
                                Suspended,
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub struct QueueConfigData {
                                pub suspend_threshold: ::core::primitive::u32,
                                pub drop_threshold: ::core::primitive::u32,
                                pub resume_threshold: ::core::primitive::u32,
                                pub threshold_weight: ::sp_weights::Weight,
                                pub weight_restrict_decay: ::sp_weights::Weight,
                                pub xcmp_max_individual_weight: ::sp_weights::Weight,
                        }
                }
                pub mod cumulus_primitives_parachain_inherent {
                        use super::runtime_types;
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub struct MessageQueueChain(pub ::subxt::utils::H256);
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub struct ParachainInherentData {
                                pub validation_data:
                                        runtime_types::polkadot_primitives::v5::PersistedValidationData<
                                                ::subxt::utils::H256,
                                                ::core::primitive::u32,
                                        >,
                                pub relay_chain_state: runtime_types::sp_trie::storage_proof::StorageProof,
                                pub downward_messages: ::std::vec::Vec<
                                        runtime_types::polkadot_core_primitives::InboundDownwardMessage<
                                                ::core::primitive::u32,
                                        >,
                                >,
                                pub horizontal_messages: ::subxt::utils::KeyedVec<
                                        runtime_types::polkadot_parachain::primitives::Id,
                                        ::std::vec::Vec<
                                                runtime_types::polkadot_core_primitives::InboundHrmpMessage<
                                                        ::core::primitive::u32,
                                                >,
                                        >,
                                >,
                        }
                }
                pub mod ethbloom {
                        use super::runtime_types;
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub struct Bloom(pub [::core::primitive::u8; 256usize]);
                }
                pub mod ethereum {
                        use super::runtime_types;
                        pub mod block {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct Block<_0> {
                                        pub header: runtime_types::ethereum::header::Header,
                                        pub transactions: ::std::vec::Vec<_0>,
                                        pub ommers: ::std::vec::Vec<runtime_types::ethereum::header::Header>,
                                }
                        }
                        pub mod header {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct Header {
                                        pub parent_hash: ::subxt::utils::H256,
                                        pub ommers_hash: ::subxt::utils::H256,
                                        pub beneficiary: ::subxt::utils::H160,
                                        pub state_root: ::subxt::utils::H256,
                                        pub transactions_root: ::subxt::utils::H256,
                                        pub receipts_root: ::subxt::utils::H256,
                                        pub logs_bloom: runtime_types::ethbloom::Bloom,
                                        pub difficulty: runtime_types::primitive_types::U256,
                                        pub number: runtime_types::primitive_types::U256,
                                        pub gas_limit: runtime_types::primitive_types::U256,
                                        pub gas_used: runtime_types::primitive_types::U256,
                                        pub timestamp: ::core::primitive::u64,
                                        pub extra_data: ::std::vec::Vec<::core::primitive::u8>,
                                        pub mix_hash: ::subxt::utils::H256,
                                        pub nonce: runtime_types::ethereum_types::hash::H64,
                                }
                        }
                        pub mod log {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct Log {
                                        pub address: ::subxt::utils::H160,
                                        pub topics: ::std::vec::Vec<::subxt::utils::H256>,
                                        pub data: ::std::vec::Vec<::core::primitive::u8>,
                                }
                        }
                        pub mod receipt {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct EIP658ReceiptData {
                                        pub status_code: ::core::primitive::u8,
                                        pub used_gas: runtime_types::primitive_types::U256,
                                        pub logs_bloom: runtime_types::ethbloom::Bloom,
                                        pub logs: ::std::vec::Vec<runtime_types::ethereum::log::Log>,
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum ReceiptV3 {
                                        #[codec(index = 0)]
                                        Legacy(runtime_types::ethereum::receipt::EIP658ReceiptData),
                                        #[codec(index = 1)]
                                        EIP2930(runtime_types::ethereum::receipt::EIP658ReceiptData),
                                        #[codec(index = 2)]
                                        EIP1559(runtime_types::ethereum::receipt::EIP658ReceiptData),
                                }
                        }
                        pub mod transaction {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct AccessListItem {
                                        pub address: ::subxt::utils::H160,
                                        pub storage_keys: ::std::vec::Vec<::subxt::utils::H256>,
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct EIP1559Transaction {
                                        pub chain_id: ::core::primitive::u64,
                                        pub nonce: runtime_types::primitive_types::U256,
                                        pub max_priority_fee_per_gas: runtime_types::primitive_types::U256,
                                        pub max_fee_per_gas: runtime_types::primitive_types::U256,
                                        pub gas_limit: runtime_types::primitive_types::U256,
                                        pub action: runtime_types::ethereum::transaction::TransactionAction,
                                        pub value: runtime_types::primitive_types::U256,
                                        pub input: ::std::vec::Vec<::core::primitive::u8>,
                                        pub access_list:
                                                ::std::vec::Vec<runtime_types::ethereum::transaction::AccessListItem>,
                                        pub odd_y_parity: ::core::primitive::bool,
                                        pub r: ::subxt::utils::H256,
                                        pub s: ::subxt::utils::H256,
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct EIP2930Transaction {
                                        pub chain_id: ::core::primitive::u64,
                                        pub nonce: runtime_types::primitive_types::U256,
                                        pub gas_price: runtime_types::primitive_types::U256,
                                        pub gas_limit: runtime_types::primitive_types::U256,
                                        pub action: runtime_types::ethereum::transaction::TransactionAction,
                                        pub value: runtime_types::primitive_types::U256,
                                        pub input: ::std::vec::Vec<::core::primitive::u8>,
                                        pub access_list:
                                                ::std::vec::Vec<runtime_types::ethereum::transaction::AccessListItem>,
                                        pub odd_y_parity: ::core::primitive::bool,
                                        pub r: ::subxt::utils::H256,
                                        pub s: ::subxt::utils::H256,
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct LegacyTransaction {
                                        pub nonce: runtime_types::primitive_types::U256,
                                        pub gas_price: runtime_types::primitive_types::U256,
                                        pub gas_limit: runtime_types::primitive_types::U256,
                                        pub action: runtime_types::ethereum::transaction::TransactionAction,
                                        pub value: runtime_types::primitive_types::U256,
                                        pub input: ::std::vec::Vec<::core::primitive::u8>,
                                        pub signature: runtime_types::ethereum::transaction::TransactionSignature,
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum TransactionAction {
                                        #[codec(index = 0)]
                                        Call(::subxt::utils::H160),
                                        #[codec(index = 1)]
                                        Create,
                                }
                                #[derive(
                                        :: codec :: Decode,
                                        :: codec :: Encode,
                                        :: subxt :: ext :: codec :: CompactAs,
                                        Clone,
                                        Debug,
                                        PartialEq,
                                )]
                                pub struct TransactionRecoveryId(pub ::core::primitive::u64);
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct TransactionSignature {
                                        pub v: runtime_types::ethereum::transaction::TransactionRecoveryId,
                                        pub r: ::subxt::utils::H256,
                                        pub s: ::subxt::utils::H256,
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum TransactionV2 {
                                        #[codec(index = 0)]
                                        Legacy(runtime_types::ethereum::transaction::LegacyTransaction),
                                        #[codec(index = 1)]
                                        EIP2930(runtime_types::ethereum::transaction::EIP2930Transaction),
                                        #[codec(index = 2)]
                                        EIP1559(runtime_types::ethereum::transaction::EIP1559Transaction),
                                }
                        }
                }
                pub mod ethereum_types {
                        use super::runtime_types;
                        pub mod hash {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct H64(pub [::core::primitive::u8; 8usize]);
                        }
                }
                pub mod evm_core {
                        use super::runtime_types;
                        pub mod error {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum ExitError {
                                        #[codec(index = 0)]
                                        StackUnderflow,
                                        #[codec(index = 1)]
                                        StackOverflow,
                                        #[codec(index = 2)]
                                        InvalidJump,
                                        #[codec(index = 3)]
                                        InvalidRange,
                                        #[codec(index = 4)]
                                        DesignatedInvalid,
                                        #[codec(index = 5)]
                                        CallTooDeep,
                                        #[codec(index = 6)]
                                        CreateCollision,
                                        #[codec(index = 7)]
                                        CreateContractLimit,
                                        #[codec(index = 15)]
                                        InvalidCode(runtime_types::evm_core::opcode::Opcode),
                                        #[codec(index = 8)]
                                        OutOfOffset,
                                        #[codec(index = 9)]
                                        OutOfGas,
                                        #[codec(index = 10)]
                                        OutOfFund,
                                        #[codec(index = 11)]
                                        PCUnderflow,
                                        #[codec(index = 12)]
                                        CreateEmpty,
                                        #[codec(index = 13)]
                                        Other(::std::string::String),
                                        #[codec(index = 14)]
                                        MaxNonce,
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum ExitFatal {
                                        #[codec(index = 0)]
                                        NotSupported,
                                        #[codec(index = 1)]
                                        UnhandledInterrupt,
                                        #[codec(index = 2)]
                                        CallErrorAsFatal(runtime_types::evm_core::error::ExitError),
                                        #[codec(index = 3)]
                                        Other(::std::string::String),
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum ExitReason {
                                        #[codec(index = 0)]
                                        Succeed(runtime_types::evm_core::error::ExitSucceed),
                                        #[codec(index = 1)]
                                        Error(runtime_types::evm_core::error::ExitError),
                                        #[codec(index = 2)]
                                        Revert(runtime_types::evm_core::error::ExitRevert),
                                        #[codec(index = 3)]
                                        Fatal(runtime_types::evm_core::error::ExitFatal),
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum ExitRevert {
                                        #[codec(index = 0)]
                                        Reverted,
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum ExitSucceed {
                                        #[codec(index = 0)]
                                        Stopped,
                                        #[codec(index = 1)]
                                        Returned,
                                        #[codec(index = 2)]
                                        Suicided,
                                }
                        }
                        pub mod opcode {
                                use super::runtime_types;
                                #[derive(
                                        :: codec :: Decode,
                                        :: codec :: Encode,
                                        :: subxt :: ext :: codec :: CompactAs,
                                        Clone,
                                        Debug,
                                        PartialEq,
                                )]
                                pub struct Opcode(pub ::core::primitive::u8);
                        }
                }
                pub mod finality_grandpa {
                        use super::runtime_types;
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub struct Commit<_0, _1, _2, _3> {
                                pub target_hash: _0,
                                pub target_number: _1,
                                pub precommits: ::std::vec::Vec<
                                        runtime_types::finality_grandpa::SignedPrecommit<_0, _1, _2, _3>,
                                >,
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub struct Precommit<_0, _1> {
                                pub target_hash: _0,
                                pub target_number: _1,
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub struct SignedPrecommit<_0, _1, _2, _3> {
                                pub precommit: runtime_types::finality_grandpa::Precommit<_0, _1>,
                                pub signature: _2,
                                pub id: _3,
                        }
                }
                pub mod fp_rpc {
                        use super::runtime_types;
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub struct TransactionStatus {
                                pub transaction_hash: ::subxt::utils::H256,
                                pub transaction_index: ::core::primitive::u32,
                                pub from: ::subxt::utils::H160,
                                pub to: ::core::option::Option<::subxt::utils::H160>,
                                pub contract_address: ::core::option::Option<::subxt::utils::H160>,
                                pub logs: ::std::vec::Vec<runtime_types::ethereum::log::Log>,
                                pub logs_bloom: runtime_types::ethbloom::Bloom,
                        }
                }
                pub mod fp_self_contained {
                        use super::runtime_types;
                        pub mod unchecked_extrinsic {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct UncheckedExtrinsic<_0, _1, _2, _3>(
                                        pub
                                                runtime_types::sp_runtime::generic::unchecked_extrinsic::UncheckedExtrinsic<
                                                        _0,
                                                        _1,
                                                        _2,
                                                        _3,
                                                >,
                                );
                        }
                }
                pub mod frame_support {
                        use super::runtime_types;
                        pub mod dispatch {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum DispatchClass {
                                        #[codec(index = 0)]
                                        Normal,
                                        #[codec(index = 1)]
                                        Operational,
                                        #[codec(index = 2)]
                                        Mandatory,
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct DispatchInfo {
                                        pub weight: ::sp_weights::Weight,
                                        pub class: runtime_types::frame_support::dispatch::DispatchClass,
                                        pub pays_fee: runtime_types::frame_support::dispatch::Pays,
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Pays {
                                        #[codec(index = 0)]
                                        Yes,
                                        #[codec(index = 1)]
                                        No,
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct PerDispatchClass<_0> {
                                        pub normal: _0,
                                        pub operational: _0,
                                        pub mandatory: _0,
                                }
                        }
                        pub mod traits {
                                use super::runtime_types;
                                pub mod tokens {
                                        use super::runtime_types;
                                        pub mod misc {
                                                use super::runtime_types;
                                                #[derive(
                                                        :: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq,
                                                )]
                                                pub enum BalanceStatus {
                                                        #[codec(index = 0)]
                                                        Free,
                                                        #[codec(index = 1)]
                                                        Reserved,
                                                }
                                        }
                                }
                        }
                }
                pub mod frame_system {
                        use super::runtime_types;
                        pub mod extensions {
                                use super::runtime_types;
                                pub mod check_genesis {
                                        use super::runtime_types;
                                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                        pub struct CheckGenesis;
                                }
                                pub mod check_mortality {
                                        use super::runtime_types;
                                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                        pub struct CheckMortality(pub ::sp_runtime::generic::Era);
                                }
                                pub mod check_non_zero_sender {
                                        use super::runtime_types;
                                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                        pub struct CheckNonZeroSender;
                                }
                                pub mod check_nonce {
                                        use super::runtime_types;
                                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                        pub struct CheckNonce(#[codec(compact)] pub ::core::primitive::u32);
                                }
                                pub mod check_spec_version {
                                        use super::runtime_types;
                                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                        pub struct CheckSpecVersion;
                                }
                                pub mod check_tx_version {
                                        use super::runtime_types;
                                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                        pub struct CheckTxVersion;
                                }
                                pub mod check_weight {
                                        use super::runtime_types;
                                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                        pub struct CheckWeight;
                                }
                        }
                        pub mod limits {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct BlockLength {
                                        pub max: runtime_types::frame_support::dispatch::PerDispatchClass<
                                                ::core::primitive::u32,
                                        >,
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct BlockWeights {
                                        pub base_block: ::sp_weights::Weight,
                                        pub max_block: ::sp_weights::Weight,
                                        pub per_class: runtime_types::frame_support::dispatch::PerDispatchClass<
                                                runtime_types::frame_system::limits::WeightsPerClass,
                                        >,
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct WeightsPerClass {
                                        pub base_extrinsic: ::sp_weights::Weight,
                                        pub max_extrinsic: ::core::option::Option<::sp_weights::Weight>,
                                        pub max_total: ::core::option::Option<::sp_weights::Weight>,
                                        pub reserved: ::core::option::Option<::sp_weights::Weight>,
                                }
                        }
                        pub mod pallet {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Call {
                                        #[codec(index = 0)]
                                        remark { remark: ::std::vec::Vec<::core::primitive::u8> },
                                        #[codec(index = 1)]
                                        set_heap_pages { pages: ::core::primitive::u64 },
                                        #[codec(index = 2)]
                                        set_code { code: ::std::vec::Vec<::core::primitive::u8> },
                                        #[codec(index = 3)]
                                        set_code_without_checks { code: ::std::vec::Vec<::core::primitive::u8> },
                                        #[codec(index = 4)]
                                        set_storage {
                                                items: ::std::vec::Vec<(
                                                        ::std::vec::Vec<::core::primitive::u8>,
                                                        ::std::vec::Vec<::core::primitive::u8>,
                                                )>,
                                        },
                                        #[codec(index = 5)]
                                        kill_storage { keys: ::std::vec::Vec<::std::vec::Vec<::core::primitive::u8>> },
                                        #[codec(index = 6)]
                                        kill_prefix {
                                                prefix: ::std::vec::Vec<::core::primitive::u8>,
                                                subkeys: ::core::primitive::u32,
                                        },
                                        #[codec(index = 7)]
                                        remark_with_event { remark: ::std::vec::Vec<::core::primitive::u8> },
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Error {
                                        #[codec(index = 0)]
                                        InvalidSpecName,
                                        #[codec(index = 1)]
                                        SpecVersionNeedsToIncrease,
                                        #[codec(index = 2)]
                                        FailedToExtractRuntimeVersion,
                                        #[codec(index = 3)]
                                        NonDefaultComposite,
                                        #[codec(index = 4)]
                                        NonZeroRefCount,
                                        #[codec(index = 5)]
                                        CallFiltered,
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Event {
                                        #[codec(index = 0)]
                                        ExtrinsicSuccess {
                                                dispatch_info: runtime_types::frame_support::dispatch::DispatchInfo,
                                        },
                                        #[codec(index = 1)]
                                        ExtrinsicFailed {
                                                dispatch_error: runtime_types::sp_runtime::DispatchError,
                                                dispatch_info: runtime_types::frame_support::dispatch::DispatchInfo,
                                        },
                                        #[codec(index = 2)]
                                        CodeUpdated,
                                        #[codec(index = 3)]
                                        NewAccount { account: ::sp_core::crypto::AccountId32 },
                                        #[codec(index = 4)]
                                        KilledAccount { account: ::sp_core::crypto::AccountId32 },
                                        #[codec(index = 5)]
                                        Remarked { sender: ::sp_core::crypto::AccountId32, hash: ::subxt::utils::H256 },
                                }
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub struct AccountInfo<_0, _1> {
                                pub nonce: _0,
                                pub consumers: _0,
                                pub providers: _0,
                                pub sufficients: _0,
                                pub data: _1,
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub struct EventRecord<_0, _1> {
                                pub phase: runtime_types::frame_system::Phase,
                                pub event: _0,
                                pub topics: ::std::vec::Vec<_1>,
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub struct LastRuntimeUpgradeInfo {
                                #[codec(compact)]
                                pub spec_version: ::core::primitive::u32,
                                pub spec_name: ::std::string::String,
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub enum Phase {
                                #[codec(index = 0)]
                                ApplyExtrinsic(::core::primitive::u32),
                                #[codec(index = 1)]
                                Finalization,
                                #[codec(index = 2)]
                                Initialization,
                        }
                }
                pub mod laos_runtime {
                        use super::runtime_types;
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub struct BridgeRejectObsoleteHeadersAndMessages;
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub struct DummyBridgeRefundEvochainMessages;
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub struct Runtime;
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub enum RuntimeCall {
                                #[codec(index = 0)]
                                System(runtime_types::frame_system::pallet::Call),
                                #[codec(index = 1)]
                                ParachainSystem(runtime_types::cumulus_pallet_parachain_system::pallet::Call),
                                #[codec(index = 2)]
                                Timestamp(runtime_types::pallet_timestamp::pallet::Call),
                                #[codec(index = 3)]
                                ParachainInfo(runtime_types::parachain_info::pallet::Call),
                                #[codec(index = 10)]
                                Balances(runtime_types::pallet_balances::pallet::Call),
                                #[codec(index = 21)]
                                CollatorSelection(runtime_types::pallet_collator_selection::pallet::Call),
                                #[codec(index = 22)]
                                Session(runtime_types::pallet_session::pallet::Call),
                                #[codec(index = 30)]
                                XcmpQueue(runtime_types::cumulus_pallet_xcmp_queue::pallet::Call),
                                #[codec(index = 31)]
                                PolkadotXcm(runtime_types::pallet_xcm::pallet::Call),
                                #[codec(index = 32)]
                                CumulusXcm(runtime_types::cumulus_pallet_xcm::pallet::Call),
                                #[codec(index = 33)]
                                DmpQueue(runtime_types::cumulus_pallet_dmp_queue::pallet::Call),
                                #[codec(index = 40)]
                                Sudo(runtime_types::pallet_sudo::pallet::Call),
                                #[codec(index = 41)]
                                CollectionManager(runtime_types::pallet_living_assets_ownership::pallet::Call),
                                #[codec(index = 50)]
                                Ethereum(runtime_types::pallet_ethereum::pallet::Call),
                                #[codec(index = 51)]
                                EVM(runtime_types::pallet_evm::pallet::Call),
                                #[codec(index = 54)]
                                BaseFee(runtime_types::pallet_base_fee::pallet::Call),
                                #[codec(index = 60)]
                                BridgeEvochainGrandpa(runtime_types::pallet_bridge_grandpa::pallet::Call),
                                #[codec(index = 61)]
                                BridgeEvochainRelayers(runtime_types::pallet_bridge_relayers::pallet::Call),
                                #[codec(index = 62)]
                                BridgeEvochainMessages(runtime_types::pallet_bridge_messages::pallet::Call),
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub enum RuntimeEvent {
                                #[codec(index = 0)]
                                System(runtime_types::frame_system::pallet::Event),
                                #[codec(index = 1)]
                                ParachainSystem(runtime_types::cumulus_pallet_parachain_system::pallet::Event),
                                #[codec(index = 10)]
                                Balances(runtime_types::pallet_balances::pallet::Event),
                                #[codec(index = 11)]
                                TransactionPayment(runtime_types::pallet_transaction_payment::pallet::Event),
                                #[codec(index = 21)]
                                CollatorSelection(runtime_types::pallet_collator_selection::pallet::Event),
                                #[codec(index = 22)]
                                Session(runtime_types::pallet_session::pallet::Event),
                                #[codec(index = 30)]
                                XcmpQueue(runtime_types::cumulus_pallet_xcmp_queue::pallet::Event),
                                #[codec(index = 31)]
                                PolkadotXcm(runtime_types::pallet_xcm::pallet::Event),
                                #[codec(index = 32)]
                                CumulusXcm(runtime_types::cumulus_pallet_xcm::pallet::Event),
                                #[codec(index = 33)]
                                DmpQueue(runtime_types::cumulus_pallet_dmp_queue::pallet::Event),
                                #[codec(index = 40)]
                                Sudo(runtime_types::pallet_sudo::pallet::Event),
                                #[codec(index = 41)]
                                CollectionManager(runtime_types::pallet_living_assets_ownership::pallet::Event),
                                #[codec(index = 50)]
                                Ethereum(runtime_types::pallet_ethereum::pallet::Event),
                                #[codec(index = 51)]
                                EVM(runtime_types::pallet_evm::pallet::Event),
                                #[codec(index = 54)]
                                BaseFee(runtime_types::pallet_base_fee::pallet::Event),
                                #[codec(index = 60)]
                                BridgeEvochainGrandpa(runtime_types::pallet_bridge_grandpa::pallet::Event),
                                #[codec(index = 61)]
                                BridgeEvochainRelayers(runtime_types::pallet_bridge_relayers::pallet::Event),
                                #[codec(index = 62)]
                                BridgeEvochainMessages(runtime_types::pallet_bridge_messages::pallet::Event),
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub struct SessionKeys {
                                pub aura: runtime_types::sp_consensus_aura::sr25519::app_sr25519::Public,
                        }
                }
                pub mod pallet_balances {
                        use super::runtime_types;
                        pub mod pallet {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Call {
                                        #[codec(index = 0)]
                                        transfer_allow_death {
                                                dest: ::subxt::utils::MultiAddress<::sp_core::crypto::AccountId32, ()>,
                                                #[codec(compact)]
                                                value: ::core::primitive::u128,
                                        },
                                        #[codec(index = 1)]
                                        set_balance_deprecated {
                                                who: ::subxt::utils::MultiAddress<::sp_core::crypto::AccountId32, ()>,
                                                #[codec(compact)]
                                                new_free: ::core::primitive::u128,
                                                #[codec(compact)]
                                                old_reserved: ::core::primitive::u128,
                                        },
                                        #[codec(index = 2)]
                                        force_transfer {
                                                source: ::subxt::utils::MultiAddress<::sp_core::crypto::AccountId32, ()>,
                                                dest: ::subxt::utils::MultiAddress<::sp_core::crypto::AccountId32, ()>,
                                                #[codec(compact)]
                                                value: ::core::primitive::u128,
                                        },
                                        #[codec(index = 3)]
                                        transfer_keep_alive {
                                                dest: ::subxt::utils::MultiAddress<::sp_core::crypto::AccountId32, ()>,
                                                #[codec(compact)]
                                                value: ::core::primitive::u128,
                                        },
                                        #[codec(index = 4)]
                                        transfer_all {
                                                dest: ::subxt::utils::MultiAddress<::sp_core::crypto::AccountId32, ()>,
                                                keep_alive: ::core::primitive::bool,
                                        },
                                        #[codec(index = 5)]
                                        force_unreserve {
                                                who: ::subxt::utils::MultiAddress<::sp_core::crypto::AccountId32, ()>,
                                                amount: ::core::primitive::u128,
                                        },
                                        #[codec(index = 6)]
                                        upgrade_accounts { who: ::std::vec::Vec<::sp_core::crypto::AccountId32> },
                                        #[codec(index = 7)]
                                        transfer {
                                                dest: ::subxt::utils::MultiAddress<::sp_core::crypto::AccountId32, ()>,
                                                #[codec(compact)]
                                                value: ::core::primitive::u128,
                                        },
                                        #[codec(index = 8)]
                                        force_set_balance {
                                                who: ::subxt::utils::MultiAddress<::sp_core::crypto::AccountId32, ()>,
                                                #[codec(compact)]
                                                new_free: ::core::primitive::u128,
                                        },
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Error {
                                        #[codec(index = 0)]
                                        VestingBalance,
                                        #[codec(index = 1)]
                                        LiquidityRestrictions,
                                        #[codec(index = 2)]
                                        InsufficientBalance,
                                        #[codec(index = 3)]
                                        ExistentialDeposit,
                                        #[codec(index = 4)]
                                        Expendability,
                                        #[codec(index = 5)]
                                        ExistingVestingSchedule,
                                        #[codec(index = 6)]
                                        DeadAccount,
                                        #[codec(index = 7)]
                                        TooManyReserves,
                                        #[codec(index = 8)]
                                        TooManyHolds,
                                        #[codec(index = 9)]
                                        TooManyFreezes,
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Event {
                                        #[codec(index = 0)]
                                        Endowed {
                                                account: ::sp_core::crypto::AccountId32,
                                                free_balance: ::core::primitive::u128,
                                        },
                                        #[codec(index = 1)]
                                        DustLost {
                                                account: ::sp_core::crypto::AccountId32,
                                                amount: ::core::primitive::u128,
                                        },
                                        #[codec(index = 2)]
                                        Transfer {
                                                from: ::sp_core::crypto::AccountId32,
                                                to: ::sp_core::crypto::AccountId32,
                                                amount: ::core::primitive::u128,
                                        },
                                        #[codec(index = 3)]
                                        BalanceSet {
                                                who: ::sp_core::crypto::AccountId32,
                                                free: ::core::primitive::u128,
                                        },
                                        #[codec(index = 4)]
                                        Reserved {
                                                who: ::sp_core::crypto::AccountId32,
                                                amount: ::core::primitive::u128,
                                        },
                                        #[codec(index = 5)]
                                        Unreserved {
                                                who: ::sp_core::crypto::AccountId32,
                                                amount: ::core::primitive::u128,
                                        },
                                        #[codec(index = 6)]
                                        ReserveRepatriated {
                                                from: ::sp_core::crypto::AccountId32,
                                                to: ::sp_core::crypto::AccountId32,
                                                amount: ::core::primitive::u128,
                                                destination_status:
                                                        runtime_types::frame_support::traits::tokens::misc::BalanceStatus,
                                        },
                                        #[codec(index = 7)]
                                        Deposit { who: ::sp_core::crypto::AccountId32, amount: ::core::primitive::u128 },
                                        #[codec(index = 8)]
                                        Withdraw {
                                                who: ::sp_core::crypto::AccountId32,
                                                amount: ::core::primitive::u128,
                                        },
                                        #[codec(index = 9)]
                                        Slashed { who: ::sp_core::crypto::AccountId32, amount: ::core::primitive::u128 },
                                        #[codec(index = 10)]
                                        Minted { who: ::sp_core::crypto::AccountId32, amount: ::core::primitive::u128 },
                                        #[codec(index = 11)]
                                        Burned { who: ::sp_core::crypto::AccountId32, amount: ::core::primitive::u128 },
                                        #[codec(index = 12)]
                                        Suspended {
                                                who: ::sp_core::crypto::AccountId32,
                                                amount: ::core::primitive::u128,
                                        },
                                        #[codec(index = 13)]
                                        Restored {
                                                who: ::sp_core::crypto::AccountId32,
                                                amount: ::core::primitive::u128,
                                        },
                                        #[codec(index = 14)]
                                        Upgraded { who: ::sp_core::crypto::AccountId32 },
                                        #[codec(index = 15)]
                                        Issued { amount: ::core::primitive::u128 },
                                        #[codec(index = 16)]
                                        Rescinded { amount: ::core::primitive::u128 },
                                        #[codec(index = 17)]
                                        Locked { who: ::sp_core::crypto::AccountId32, amount: ::core::primitive::u128 },
                                        #[codec(index = 18)]
                                        Unlocked {
                                                who: ::sp_core::crypto::AccountId32,
                                                amount: ::core::primitive::u128,
                                        },
                                        #[codec(index = 19)]
                                        Frozen { who: ::sp_core::crypto::AccountId32, amount: ::core::primitive::u128 },
                                        #[codec(index = 20)]
                                        Thawed { who: ::sp_core::crypto::AccountId32, amount: ::core::primitive::u128 },
                                }
                        }
                        pub mod types {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct AccountData<_0> {
                                        pub free: _0,
                                        pub reserved: _0,
                                        pub frozen: _0,
                                        pub flags: runtime_types::pallet_balances::types::ExtraFlags,
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct BalanceLock<_0> {
                                        pub id: [::core::primitive::u8; 8usize],
                                        pub amount: _0,
                                        pub reasons: runtime_types::pallet_balances::types::Reasons,
                                }
                                #[derive(
                                        :: codec :: Decode,
                                        :: codec :: Encode,
                                        :: subxt :: ext :: codec :: CompactAs,
                                        Clone,
                                        Debug,
                                        PartialEq,
                                )]
                                pub struct ExtraFlags(pub ::core::primitive::u128);
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct IdAmount<_0, _1> {
                                        pub id: _0,
                                        pub amount: _1,
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Reasons {
                                        #[codec(index = 0)]
                                        Fee,
                                        #[codec(index = 1)]
                                        Misc,
                                        #[codec(index = 2)]
                                        All,
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct ReserveData<_0, _1> {
                                        pub id: _0,
                                        pub amount: _1,
                                }
                        }
                }
                pub mod pallet_base_fee {
                        use super::runtime_types;
                        pub mod pallet {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Call {
                                        #[codec(index = 0)]
                                        set_base_fee_per_gas { fee: runtime_types::primitive_types::U256 },
                                        #[codec(index = 1)]
                                        set_elasticity { elasticity: runtime_types::sp_arithmetic::per_things::Permill },
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Event {
                                        #[codec(index = 0)]
                                        NewBaseFeePerGas { fee: runtime_types::primitive_types::U256 },
                                        #[codec(index = 1)]
                                        BaseFeeOverflow,
                                        #[codec(index = 2)]
                                        NewElasticity { elasticity: runtime_types::sp_arithmetic::per_things::Permill },
                                }
                        }
                }
                pub mod pallet_bridge_grandpa {
                        use super::runtime_types;
                        pub mod pallet {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Call {
                                        #[codec(index = 0)]
                                        submit_finality_proof {
                                                finality_target: ::std::boxed::Box<
                                                        ::sp_runtime::generic::Header<
                                                                ::core::primitive::u32,
                                                                ::ep_bridge::Hasher,
                                                        >,
                                                >,
                                                justification: ::bp_header_chain::justification::GrandpaJustification<
                                                        ::sp_runtime::generic::Header<
                                                                ::core::primitive::u32,
                                                                ::ep_bridge::Hasher,
                                                        >,
                                                >,
                                        },
                                        #[codec(index = 1)]
                                        initialize {
                                                init_data: ::bp_header_chain::InitializationData<
                                                        ::sp_runtime::generic::Header<
                                                                ::core::primitive::u32,
                                                                ::ep_bridge::Hasher,
                                                        >,
                                                >,
                                        },
                                        #[codec(index = 2)]
                                        set_owner { new_owner: ::core::option::Option<::sp_core::crypto::AccountId32> },
                                        #[codec(index = 3)]
                                        set_operating_mode {
                                                operating_mode: runtime_types::bp_runtime::BasicOperatingMode,
                                        },
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Error {
                                        #[codec(index = 0)]
                                        InvalidJustification,
                                        #[codec(index = 1)]
                                        InvalidAuthoritySet,
                                        #[codec(index = 2)]
                                        OldHeader,
                                        #[codec(index = 3)]
                                        UnsupportedScheduledChange,
                                        #[codec(index = 4)]
                                        NotInitialized,
                                        #[codec(index = 5)]
                                        AlreadyInitialized,
                                        #[codec(index = 6)]
                                        TooManyAuthoritiesInSet,
                                        #[codec(index = 7)]
                                        BridgeModule(runtime_types::bp_runtime::OwnedBridgeModuleError),
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Event {
                                        #[codec(index = 0)]
                                        UpdatedBestFinalizedHeader {
                                                number: ::core::primitive::u32,
                                                hash: runtime_types::ep_bridge::evo_hash::EvoHash,
                                                justification: ::bp_header_chain::justification::GrandpaJustification<
                                                        ::sp_runtime::generic::Header<
                                                                ::core::primitive::u32,
                                                                ::ep_bridge::Hasher,
                                                        >,
                                                >,
                                        },
                                }
                        }
                        pub mod storage_types {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct StoredAuthoritySet {
                                        pub authorities: runtime_types::bounded_collections::bounded_vec::BoundedVec<(
                                                runtime_types::sp_consensus_grandpa::app::Public,
                                                ::core::primitive::u64,
                                        )>,
                                        pub set_id: ::core::primitive::u64,
                                }
                        }
                }
                pub mod pallet_collator_selection {
                        use super::runtime_types;
                        pub mod pallet {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Call {
                                        #[codec(index = 0)]
                                        set_invulnerables { new: ::std::vec::Vec<::sp_core::crypto::AccountId32> },
                                        #[codec(index = 1)]
                                        set_desired_candidates { max: ::core::primitive::u32 },
                                        #[codec(index = 2)]
                                        set_candidacy_bond { bond: ::core::primitive::u128 },
                                        #[codec(index = 3)]
                                        register_as_candidate,
                                        #[codec(index = 4)]
                                        leave_intent,
                                        #[codec(index = 5)]
                                        add_invulnerable { who: ::sp_core::crypto::AccountId32 },
                                        #[codec(index = 6)]
                                        remove_invulnerable { who: ::sp_core::crypto::AccountId32 },
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct CandidateInfo<_0, _1> {
                                        pub who: _0,
                                        pub deposit: _1,
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Error {
                                        #[codec(index = 0)]
                                        TooManyCandidates,
                                        #[codec(index = 1)]
                                        TooFewEligibleCollators,
                                        #[codec(index = 2)]
                                        AlreadyCandidate,
                                        #[codec(index = 3)]
                                        NotCandidate,
                                        #[codec(index = 4)]
                                        TooManyInvulnerables,
                                        #[codec(index = 5)]
                                        AlreadyInvulnerable,
                                        #[codec(index = 6)]
                                        NotInvulnerable,
                                        #[codec(index = 7)]
                                        NoAssociatedValidatorId,
                                        #[codec(index = 8)]
                                        ValidatorNotRegistered,
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Event {
                                        #[codec(index = 0)]
                                        NewInvulnerables {
                                                invulnerables: ::std::vec::Vec<::sp_core::crypto::AccountId32>,
                                        },
                                        #[codec(index = 1)]
                                        InvulnerableAdded { account_id: ::sp_core::crypto::AccountId32 },
                                        #[codec(index = 2)]
                                        InvulnerableRemoved { account_id: ::sp_core::crypto::AccountId32 },
                                        #[codec(index = 3)]
                                        NewDesiredCandidates { desired_candidates: ::core::primitive::u32 },
                                        #[codec(index = 4)]
                                        NewCandidacyBond { bond_amount: ::core::primitive::u128 },
                                        #[codec(index = 5)]
                                        CandidateAdded {
                                                account_id: ::sp_core::crypto::AccountId32,
                                                deposit: ::core::primitive::u128,
                                        },
                                        #[codec(index = 6)]
                                        CandidateRemoved { account_id: ::sp_core::crypto::AccountId32 },
                                        #[codec(index = 7)]
                                        InvalidInvulnerableSkipped { account_id: ::sp_core::crypto::AccountId32 },
                                }
                        }
                }
                pub mod pallet_ethereum {
                        use super::runtime_types;
                        pub mod pallet {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Call {
                                        #[codec(index = 0)]
                                        transact { transaction: runtime_types::ethereum::transaction::TransactionV2 },
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Error {
                                        #[codec(index = 0)]
                                        InvalidSignature,
                                        #[codec(index = 1)]
                                        PreLogExists,
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Event {
                                        #[codec(index = 0)]
                                        Executed {
                                                from: ::subxt::utils::H160,
                                                to: ::subxt::utils::H160,
                                                transaction_hash: ::subxt::utils::H256,
                                                exit_reason: runtime_types::evm_core::error::ExitReason,
                                                extra_data: ::std::vec::Vec<::core::primitive::u8>,
                                        },
                                }
                        }
                }
                pub mod pallet_evm {
                        use super::runtime_types;
                        pub mod pallet {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Call {
                                        #[codec(index = 0)]
                                        withdraw { address: ::subxt::utils::H160, value: ::core::primitive::u128 },
                                        #[codec(index = 1)]
                                        call {
                                                source: ::subxt::utils::H160,
                                                target: ::subxt::utils::H160,
                                                input: ::std::vec::Vec<::core::primitive::u8>,
                                                value: runtime_types::primitive_types::U256,
                                                gas_limit: ::core::primitive::u64,
                                                max_fee_per_gas: runtime_types::primitive_types::U256,
                                                max_priority_fee_per_gas:
                                                        ::core::option::Option<runtime_types::primitive_types::U256>,
                                                nonce: ::core::option::Option<runtime_types::primitive_types::U256>,
                                                access_list: ::std::vec::Vec<(
                                                        ::subxt::utils::H160,
                                                        ::std::vec::Vec<::subxt::utils::H256>,
                                                )>,
                                        },
                                        #[codec(index = 2)]
                                        create {
                                                source: ::subxt::utils::H160,
                                                init: ::std::vec::Vec<::core::primitive::u8>,
                                                value: runtime_types::primitive_types::U256,
                                                gas_limit: ::core::primitive::u64,
                                                max_fee_per_gas: runtime_types::primitive_types::U256,
                                                max_priority_fee_per_gas:
                                                        ::core::option::Option<runtime_types::primitive_types::U256>,
                                                nonce: ::core::option::Option<runtime_types::primitive_types::U256>,
                                                access_list: ::std::vec::Vec<(
                                                        ::subxt::utils::H160,
                                                        ::std::vec::Vec<::subxt::utils::H256>,
                                                )>,
                                        },
                                        #[codec(index = 3)]
                                        create2 {
                                                source: ::subxt::utils::H160,
                                                init: ::std::vec::Vec<::core::primitive::u8>,
                                                salt: ::subxt::utils::H256,
                                                value: runtime_types::primitive_types::U256,
                                                gas_limit: ::core::primitive::u64,
                                                max_fee_per_gas: runtime_types::primitive_types::U256,
                                                max_priority_fee_per_gas:
                                                        ::core::option::Option<runtime_types::primitive_types::U256>,
                                                nonce: ::core::option::Option<runtime_types::primitive_types::U256>,
                                                access_list: ::std::vec::Vec<(
                                                        ::subxt::utils::H160,
                                                        ::std::vec::Vec<::subxt::utils::H256>,
                                                )>,
                                        },
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Error {
                                        #[codec(index = 0)]
                                        BalanceLow,
                                        #[codec(index = 1)]
                                        FeeOverflow,
                                        #[codec(index = 2)]
                                        PaymentOverflow,
                                        #[codec(index = 3)]
                                        WithdrawFailed,
                                        #[codec(index = 4)]
                                        GasPriceTooLow,
                                        #[codec(index = 5)]
                                        InvalidNonce,
                                        #[codec(index = 6)]
                                        GasLimitTooLow,
                                        #[codec(index = 7)]
                                        GasLimitTooHigh,
                                        #[codec(index = 8)]
                                        Undefined,
                                        #[codec(index = 9)]
                                        Reentrancy,
                                        #[codec(index = 10)]
                                        TransactionMustComeFromEOA,
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Event {
                                        #[codec(index = 0)]
                                        Log { log: runtime_types::ethereum::log::Log },
                                        #[codec(index = 1)]
                                        Created { address: ::subxt::utils::H160 },
                                        #[codec(index = 2)]
                                        CreatedFailed { address: ::subxt::utils::H160 },
                                        #[codec(index = 3)]
                                        Executed { address: ::subxt::utils::H160 },
                                        #[codec(index = 4)]
                                        ExecutedFailed { address: ::subxt::utils::H160 },
                                }
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub struct CodeMetadata {
                                pub size: ::core::primitive::u64,
                                pub hash: ::subxt::utils::H256,
                        }
                }
                pub mod pallet_living_assets_ownership {
                        use super::runtime_types;
                        pub mod pallet {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Call {
                                        #[codec(index = 0)]
                                        create_collection,
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Error {
                                        #[codec(index = 0)]
                                        CollectionAlreadyExists,
                                        #[codec(index = 1)]
                                        CollectionIdOverflow,
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Event {
                                        #[codec(index = 0)]
                                        CollectionCreated {
                                                collection_id: ::core::primitive::u64,
                                                who: ::sp_core::crypto::AccountId32,
                                        },
                                }
                        }
                }
                pub mod pallet_session {
                        use super::runtime_types;
                        pub mod pallet {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Call {
                                        #[codec(index = 0)]
                                        set_keys {
                                                keys: runtime_types::laos_runtime::SessionKeys,
                                                proof: ::std::vec::Vec<::core::primitive::u8>,
                                        },
                                        #[codec(index = 1)]
                                        purge_keys,
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Error {
                                        #[codec(index = 0)]
                                        InvalidProof,
                                        #[codec(index = 1)]
                                        NoAssociatedValidatorId,
                                        #[codec(index = 2)]
                                        DuplicatedKey,
                                        #[codec(index = 3)]
                                        NoKeys,
                                        #[codec(index = 4)]
                                        NoAccount,
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Event {
                                        #[codec(index = 0)]
                                        NewSession { session_index: ::core::primitive::u32 },
                                }
                        }
                }
                pub mod pallet_sudo {
                        use super::runtime_types;
                        pub mod pallet {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Call {
                                        #[codec(index = 0)]
                                        sudo { call: ::std::boxed::Box<runtime_types::laos_runtime::RuntimeCall> },
                                        #[codec(index = 1)]
                                        sudo_unchecked_weight {
                                                call: ::std::boxed::Box<runtime_types::laos_runtime::RuntimeCall>,
                                                weight: ::sp_weights::Weight,
                                        },
                                        #[codec(index = 2)]
                                        set_key {
                                                new: ::subxt::utils::MultiAddress<::sp_core::crypto::AccountId32, ()>,
                                        },
                                        #[codec(index = 3)]
                                        sudo_as {
                                                who: ::subxt::utils::MultiAddress<::sp_core::crypto::AccountId32, ()>,
                                                call: ::std::boxed::Box<runtime_types::laos_runtime::RuntimeCall>,
                                        },
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Error {
                                        #[codec(index = 0)]
                                        RequireSudo,
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Event {
                                        #[codec(index = 0)]
                                        Sudid {
                                                sudo_result:
                                                        ::core::result::Result<(), runtime_types::sp_runtime::DispatchError>,
                                        },
                                        #[codec(index = 1)]
                                        KeyChanged {
                                                old_sudoer: ::core::option::Option<::sp_core::crypto::AccountId32>,
                                        },
                                        #[codec(index = 2)]
                                        SudoAsDone {
                                                sudo_result:
                                                        ::core::result::Result<(), runtime_types::sp_runtime::DispatchError>,
                                        },
                                }
                        }
                }
                pub mod pallet_timestamp {
                        use super::runtime_types;
                        pub mod pallet {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Call {
                                        #[codec(index = 0)]
                                        set {
                                                #[codec(compact)]
                                                now: ::core::primitive::u64,
                                        },
                                }
                        }
                }
                pub mod pallet_transaction_payment {
                        use super::runtime_types;
                        pub mod pallet {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Event {
                                        #[codec(index = 0)]
                                        TransactionFeePaid {
                                                who: ::sp_core::crypto::AccountId32,
                                                actual_fee: ::core::primitive::u128,
                                                tip: ::core::primitive::u128,
                                        },
                                }
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub struct ChargeTransactionPayment(#[codec(compact)] pub ::core::primitive::u128);
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub enum Releases {
                                #[codec(index = 0)]
                                V1Ancient,
                                #[codec(index = 1)]
                                V2,
                        }
                }
                pub mod pallet_xcm {
                        use super::runtime_types;
                        pub mod pallet {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Call {
                                        #[codec(index = 0)]
                                        send {
                                                dest: ::std::boxed::Box<runtime_types::xcm::VersionedMultiLocation>,
                                                message: ::std::boxed::Box<runtime_types::xcm::VersionedXcm>,
                                        },
                                        #[codec(index = 1)]
                                        teleport_assets {
                                                dest: ::std::boxed::Box<runtime_types::xcm::VersionedMultiLocation>,
                                                beneficiary: ::std::boxed::Box<runtime_types::xcm::VersionedMultiLocation>,
                                                assets: ::std::boxed::Box<runtime_types::xcm::VersionedMultiAssets>,
                                                fee_asset_item: ::core::primitive::u32,
                                        },
                                        #[codec(index = 2)]
                                        reserve_transfer_assets {
                                                dest: ::std::boxed::Box<runtime_types::xcm::VersionedMultiLocation>,
                                                beneficiary: ::std::boxed::Box<runtime_types::xcm::VersionedMultiLocation>,
                                                assets: ::std::boxed::Box<runtime_types::xcm::VersionedMultiAssets>,
                                                fee_asset_item: ::core::primitive::u32,
                                        },
                                        #[codec(index = 3)]
                                        execute {
                                                message: ::std::boxed::Box<runtime_types::xcm::VersionedXcm>,
                                                max_weight: ::sp_weights::Weight,
                                        },
                                        #[codec(index = 4)]
                                        force_xcm_version {
                                                location:
                                                        ::std::boxed::Box<runtime_types::xcm::v3::multilocation::MultiLocation>,
                                                version: ::core::primitive::u32,
                                        },
                                        #[codec(index = 5)]
                                        force_default_xcm_version {
                                                maybe_xcm_version: ::core::option::Option<::core::primitive::u32>,
                                        },
                                        #[codec(index = 6)]
                                        force_subscribe_version_notify {
                                                location: ::std::boxed::Box<runtime_types::xcm::VersionedMultiLocation>,
                                        },
                                        #[codec(index = 7)]
                                        force_unsubscribe_version_notify {
                                                location: ::std::boxed::Box<runtime_types::xcm::VersionedMultiLocation>,
                                        },
                                        #[codec(index = 8)]
                                        limited_reserve_transfer_assets {
                                                dest: ::std::boxed::Box<runtime_types::xcm::VersionedMultiLocation>,
                                                beneficiary: ::std::boxed::Box<runtime_types::xcm::VersionedMultiLocation>,
                                                assets: ::std::boxed::Box<runtime_types::xcm::VersionedMultiAssets>,
                                                fee_asset_item: ::core::primitive::u32,
                                                weight_limit: runtime_types::xcm::v3::WeightLimit,
                                        },
                                        #[codec(index = 9)]
                                        limited_teleport_assets {
                                                dest: ::std::boxed::Box<runtime_types::xcm::VersionedMultiLocation>,
                                                beneficiary: ::std::boxed::Box<runtime_types::xcm::VersionedMultiLocation>,
                                                assets: ::std::boxed::Box<runtime_types::xcm::VersionedMultiAssets>,
                                                fee_asset_item: ::core::primitive::u32,
                                                weight_limit: runtime_types::xcm::v3::WeightLimit,
                                        },
                                        #[codec(index = 10)]
                                        force_suspension { suspended: ::core::primitive::bool },
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Error {
                                        #[codec(index = 0)]
                                        Unreachable,
                                        #[codec(index = 1)]
                                        SendFailure,
                                        #[codec(index = 2)]
                                        Filtered,
                                        #[codec(index = 3)]
                                        UnweighableMessage,
                                        #[codec(index = 4)]
                                        DestinationNotInvertible,
                                        #[codec(index = 5)]
                                        Empty,
                                        #[codec(index = 6)]
                                        CannotReanchor,
                                        #[codec(index = 7)]
                                        TooManyAssets,
                                        #[codec(index = 8)]
                                        InvalidOrigin,
                                        #[codec(index = 9)]
                                        BadVersion,
                                        #[codec(index = 10)]
                                        BadLocation,
                                        #[codec(index = 11)]
                                        NoSubscription,
                                        #[codec(index = 12)]
                                        AlreadySubscribed,
                                        #[codec(index = 13)]
                                        InvalidAsset,
                                        #[codec(index = 14)]
                                        LowBalance,
                                        #[codec(index = 15)]
                                        TooManyLocks,
                                        #[codec(index = 16)]
                                        AccountNotSovereign,
                                        #[codec(index = 17)]
                                        FeesNotMet,
                                        #[codec(index = 18)]
                                        LockNotFound,
                                        #[codec(index = 19)]
                                        InUse,
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Event {
                                        #[codec(index = 0)]
                                        Attempted { outcome: runtime_types::xcm::v3::traits::Outcome },
                                        #[codec(index = 1)]
                                        Sent {
                                                origin: runtime_types::xcm::v3::multilocation::MultiLocation,
                                                destination: runtime_types::xcm::v3::multilocation::MultiLocation,
                                                message: runtime_types::xcm::v3::Xcm,
                                                message_id: [::core::primitive::u8; 32usize],
                                        },
                                        #[codec(index = 2)]
                                        UnexpectedResponse {
                                                origin: runtime_types::xcm::v3::multilocation::MultiLocation,
                                                query_id: ::core::primitive::u64,
                                        },
                                        #[codec(index = 3)]
                                        ResponseReady {
                                                query_id: ::core::primitive::u64,
                                                response: runtime_types::xcm::v3::Response,
                                        },
                                        #[codec(index = 4)]
                                        Notified {
                                                query_id: ::core::primitive::u64,
                                                pallet_index: ::core::primitive::u8,
                                                call_index: ::core::primitive::u8,
                                        },
                                        #[codec(index = 5)]
                                        NotifyOverweight {
                                                query_id: ::core::primitive::u64,
                                                pallet_index: ::core::primitive::u8,
                                                call_index: ::core::primitive::u8,
                                                actual_weight: ::sp_weights::Weight,
                                                max_budgeted_weight: ::sp_weights::Weight,
                                        },
                                        #[codec(index = 6)]
                                        NotifyDispatchError {
                                                query_id: ::core::primitive::u64,
                                                pallet_index: ::core::primitive::u8,
                                                call_index: ::core::primitive::u8,
                                        },
                                        #[codec(index = 7)]
                                        NotifyDecodeFailed {
                                                query_id: ::core::primitive::u64,
                                                pallet_index: ::core::primitive::u8,
                                                call_index: ::core::primitive::u8,
                                        },
                                        #[codec(index = 8)]
                                        InvalidResponder {
                                                origin: runtime_types::xcm::v3::multilocation::MultiLocation,
                                                query_id: ::core::primitive::u64,
                                                expected_location: ::core::option::Option<
                                                        runtime_types::xcm::v3::multilocation::MultiLocation,
                                                >,
                                        },
                                        #[codec(index = 9)]
                                        InvalidResponderVersion {
                                                origin: runtime_types::xcm::v3::multilocation::MultiLocation,
                                                query_id: ::core::primitive::u64,
                                        },
                                        #[codec(index = 10)]
                                        ResponseTaken { query_id: ::core::primitive::u64 },
                                        #[codec(index = 11)]
                                        AssetsTrapped {
                                                hash: ::subxt::utils::H256,
                                                origin: runtime_types::xcm::v3::multilocation::MultiLocation,
                                                assets: runtime_types::xcm::VersionedMultiAssets,
                                        },
                                        #[codec(index = 12)]
                                        VersionChangeNotified {
                                                destination: runtime_types::xcm::v3::multilocation::MultiLocation,
                                                result: ::core::primitive::u32,
                                                cost: runtime_types::xcm::v3::multiasset::MultiAssets,
                                                message_id: [::core::primitive::u8; 32usize],
                                        },
                                        #[codec(index = 13)]
                                        SupportedVersionChanged {
                                                location: runtime_types::xcm::v3::multilocation::MultiLocation,
                                                version: ::core::primitive::u32,
                                        },
                                        #[codec(index = 14)]
                                        NotifyTargetSendFail {
                                                location: runtime_types::xcm::v3::multilocation::MultiLocation,
                                                query_id: ::core::primitive::u64,
                                                error: runtime_types::xcm::v3::traits::Error,
                                        },
                                        #[codec(index = 15)]
                                        NotifyTargetMigrationFail {
                                                location: runtime_types::xcm::VersionedMultiLocation,
                                                query_id: ::core::primitive::u64,
                                        },
                                        #[codec(index = 16)]
                                        InvalidQuerierVersion {
                                                origin: runtime_types::xcm::v3::multilocation::MultiLocation,
                                                query_id: ::core::primitive::u64,
                                        },
                                        #[codec(index = 17)]
                                        InvalidQuerier {
                                                origin: runtime_types::xcm::v3::multilocation::MultiLocation,
                                                query_id: ::core::primitive::u64,
                                                expected_querier: runtime_types::xcm::v3::multilocation::MultiLocation,
                                                maybe_actual_querier: ::core::option::Option<
                                                        runtime_types::xcm::v3::multilocation::MultiLocation,
                                                >,
                                        },
                                        #[codec(index = 18)]
                                        VersionNotifyStarted {
                                                destination: runtime_types::xcm::v3::multilocation::MultiLocation,
                                                cost: runtime_types::xcm::v3::multiasset::MultiAssets,
                                                message_id: [::core::primitive::u8; 32usize],
                                        },
                                        #[codec(index = 19)]
                                        VersionNotifyRequested {
                                                destination: runtime_types::xcm::v3::multilocation::MultiLocation,
                                                cost: runtime_types::xcm::v3::multiasset::MultiAssets,
                                                message_id: [::core::primitive::u8; 32usize],
                                        },
                                        #[codec(index = 20)]
                                        VersionNotifyUnrequested {
                                                destination: runtime_types::xcm::v3::multilocation::MultiLocation,
                                                cost: runtime_types::xcm::v3::multiasset::MultiAssets,
                                                message_id: [::core::primitive::u8; 32usize],
                                        },
                                        #[codec(index = 21)]
                                        FeesPaid {
                                                paying: runtime_types::xcm::v3::multilocation::MultiLocation,
                                                fees: runtime_types::xcm::v3::multiasset::MultiAssets,
                                        },
                                        #[codec(index = 22)]
                                        AssetsClaimed {
                                                hash: ::subxt::utils::H256,
                                                origin: runtime_types::xcm::v3::multilocation::MultiLocation,
                                                assets: runtime_types::xcm::VersionedMultiAssets,
                                        },
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum QueryStatus<_0> {
                                        #[codec(index = 0)]
                                        Pending {
                                                responder: runtime_types::xcm::VersionedMultiLocation,
                                                maybe_match_querier:
                                                        ::core::option::Option<runtime_types::xcm::VersionedMultiLocation>,
                                                maybe_notify:
                                                        ::core::option::Option<(::core::primitive::u8, ::core::primitive::u8)>,
                                                timeout: _0,
                                        },
                                        #[codec(index = 1)]
                                        VersionNotifier {
                                                origin: runtime_types::xcm::VersionedMultiLocation,
                                                is_active: ::core::primitive::bool,
                                        },
                                        #[codec(index = 2)]
                                        Ready { response: runtime_types::xcm::VersionedResponse, at: _0 },
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct RemoteLockedFungibleRecord<_0> {
                                        pub amount: ::core::primitive::u128,
                                        pub owner: runtime_types::xcm::VersionedMultiLocation,
                                        pub locker: runtime_types::xcm::VersionedMultiLocation,
                                        pub consumers: runtime_types::bounded_collections::bounded_vec::BoundedVec<(
                                                _0,
                                                ::core::primitive::u128,
                                        )>,
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum VersionMigrationStage {
                                        #[codec(index = 0)]
                                        MigrateSupportedVersion,
                                        #[codec(index = 1)]
                                        MigrateVersionNotifiers,
                                        #[codec(index = 2)]
                                        NotifyCurrentTargets(
                                                ::core::option::Option<::std::vec::Vec<::core::primitive::u8>>,
                                        ),
                                        #[codec(index = 3)]
                                        MigrateAndNotifyOldTargets,
                                }
                        }
                }
                pub mod parachain_info {
                        use super::runtime_types;
                        pub mod pallet {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Call {}
                        }
                }
                pub mod polkadot_core_primitives {
                        use super::runtime_types;
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub struct InboundDownwardMessage<_0> {
                                pub sent_at: _0,
                                pub msg: ::std::vec::Vec<::core::primitive::u8>,
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub struct InboundHrmpMessage<_0> {
                                pub sent_at: _0,
                                pub data: ::std::vec::Vec<::core::primitive::u8>,
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub struct OutboundHrmpMessage<_0> {
                                pub recipient: _0,
                                pub data: ::std::vec::Vec<::core::primitive::u8>,
                        }
                }
                pub mod polkadot_parachain {
                        use super::runtime_types;
                        pub mod primitives {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct HeadData(pub ::std::vec::Vec<::core::primitive::u8>);
                                #[derive(
                                        :: codec :: Decode,
                                        :: codec :: Encode,
                                        :: subxt :: ext :: codec :: CompactAs,
                                        Clone,
                                        Debug,
                                        PartialEq,
                                )]
                                pub struct Id(pub ::core::primitive::u32);
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum XcmpMessageFormat {
                                        #[codec(index = 0)]
                                        ConcatenatedVersionedXcm,
                                        #[codec(index = 1)]
                                        ConcatenatedEncodedBlob,
                                        #[codec(index = 2)]
                                        Signals,
                                }
                        }
                }
                pub mod polkadot_primitives {
                        use super::runtime_types;
                        pub mod v5 {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct AbridgedHostConfiguration {
                                        pub max_code_size: ::core::primitive::u32,
                                        pub max_head_data_size: ::core::primitive::u32,
                                        pub max_upward_queue_count: ::core::primitive::u32,
                                        pub max_upward_queue_size: ::core::primitive::u32,
                                        pub max_upward_message_size: ::core::primitive::u32,
                                        pub max_upward_message_num_per_candidate: ::core::primitive::u32,
                                        pub hrmp_max_message_num_per_candidate: ::core::primitive::u32,
                                        pub validation_upgrade_cooldown: ::core::primitive::u32,
                                        pub validation_upgrade_delay: ::core::primitive::u32,
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct AbridgedHrmpChannel {
                                        pub max_capacity: ::core::primitive::u32,
                                        pub max_total_size: ::core::primitive::u32,
                                        pub max_message_size: ::core::primitive::u32,
                                        pub msg_count: ::core::primitive::u32,
                                        pub total_size: ::core::primitive::u32,
                                        pub mqc_head: ::core::option::Option<::subxt::utils::H256>,
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct PersistedValidationData<_0, _1> {
                                        pub parent_head: runtime_types::polkadot_parachain::primitives::HeadData,
                                        pub relay_parent_number: _1,
                                        pub relay_parent_storage_root: _0,
                                        pub max_pov_size: _1,
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum UpgradeRestriction {
                                        #[codec(index = 0)]
                                        Present,
                                }
                        }
                }
                pub mod primitive_types {
                        use super::runtime_types;
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub struct U256(pub [::core::primitive::u64; 4usize]);
                }
                pub mod sp_arithmetic {
                        use super::runtime_types;
                        pub mod fixed_point {
                                use super::runtime_types;
                                #[derive(
                                        :: codec :: Decode,
                                        :: codec :: Encode,
                                        :: subxt :: ext :: codec :: CompactAs,
                                        Clone,
                                        Debug,
                                        PartialEq,
                                )]
                                pub struct FixedU128(pub ::core::primitive::u128);
                        }
                        pub mod per_things {
                                use super::runtime_types;
                                #[derive(
                                        :: codec :: Decode,
                                        :: codec :: Encode,
                                        :: subxt :: ext :: codec :: CompactAs,
                                        Clone,
                                        Debug,
                                        PartialEq,
                                )]
                                pub struct Permill(pub ::core::primitive::u32);
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub enum ArithmeticError {
                                #[codec(index = 0)]
                                Underflow,
                                #[codec(index = 1)]
                                Overflow,
                                #[codec(index = 2)]
                                DivisionByZero,
                        }
                }
                pub mod sp_consensus_aura {
                        use super::runtime_types;
                        pub mod sr25519 {
                                use super::runtime_types;
                                pub mod app_sr25519 {
                                        use super::runtime_types;
                                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                        pub struct Public(pub runtime_types::sp_core::sr25519::Public);
                                }
                        }
                }
                pub mod sp_consensus_grandpa {
                        use super::runtime_types;
                        pub mod app {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct Public(pub runtime_types::sp_core::ed25519::Public);
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct Signature(pub runtime_types::sp_core::ed25519::Signature);
                        }
                }
                pub mod sp_consensus_slots {
                        use super::runtime_types;
                        #[derive(
                                :: codec :: Decode,
                                :: codec :: Encode,
                                :: subxt :: ext :: codec :: CompactAs,
                                Clone,
                                Debug,
                                PartialEq,
                        )]
                        pub struct Slot(pub ::core::primitive::u64);
                }
                pub mod sp_core {
                        use super::runtime_types;
                        pub mod crypto {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct KeyTypeId(pub [::core::primitive::u8; 4usize]);
                        }
                        pub mod ecdsa {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct Signature(pub [::core::primitive::u8; 65usize]);
                        }
                        pub mod ed25519 {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct Public(pub [::core::primitive::u8; 32usize]);
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct Signature(pub [::core::primitive::u8; 64usize]);
                        }
                        pub mod sr25519 {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct Public(pub [::core::primitive::u8; 32usize]);
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct Signature(pub [::core::primitive::u8; 64usize]);
                        }
                }
                pub mod sp_runtime {
                        use super::runtime_types;
                        pub mod generic {
                                use super::runtime_types;
                                pub mod digest {
                                        use super::runtime_types;
                                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                        pub enum DigestItem {
                                                #[codec(index = 6)]
                                                PreRuntime(
                                                        [::core::primitive::u8; 4usize],
                                                        ::std::vec::Vec<::core::primitive::u8>,
                                                ),
                                                #[codec(index = 4)]
                                                Consensus(
                                                        [::core::primitive::u8; 4usize],
                                                        ::std::vec::Vec<::core::primitive::u8>,
                                                ),
                                                #[codec(index = 5)]
                                                Seal(
                                                        [::core::primitive::u8; 4usize],
                                                        ::std::vec::Vec<::core::primitive::u8>,
                                                ),
                                                #[codec(index = 0)]
                                                Other(::std::vec::Vec<::core::primitive::u8>),
                                                #[codec(index = 8)]
                                                RuntimeEnvironmentUpdated,
                                        }
                                }
                                pub mod unchecked_extrinsic {
                                        use super::runtime_types;
                                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                        pub struct UncheckedExtrinsic<_0, _1, _2, _3>(
                                                pub ::std::vec::Vec<::core::primitive::u8>,
                                                #[codec(skip)] pub ::core::marker::PhantomData<(_0, _1, _2, _3)>,
                                        );
                                }
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub enum DispatchError {
                                #[codec(index = 0)]
                                Other,
                                #[codec(index = 1)]
                                CannotLookup,
                                #[codec(index = 2)]
                                BadOrigin,
                                #[codec(index = 3)]
                                Module(runtime_types::sp_runtime::ModuleError),
                                #[codec(index = 4)]
                                ConsumerRemaining,
                                #[codec(index = 5)]
                                NoProviders,
                                #[codec(index = 6)]
                                TooManyConsumers,
                                #[codec(index = 7)]
                                Token(runtime_types::sp_runtime::TokenError),
                                #[codec(index = 8)]
                                Arithmetic(runtime_types::sp_arithmetic::ArithmeticError),
                                #[codec(index = 9)]
                                Transactional(runtime_types::sp_runtime::TransactionalError),
                                #[codec(index = 10)]
                                Exhausted,
                                #[codec(index = 11)]
                                Corruption,
                                #[codec(index = 12)]
                                Unavailable,
                                #[codec(index = 13)]
                                RootNotAllowed,
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub struct ModuleError {
                                pub index: ::core::primitive::u8,
                                pub error: [::core::primitive::u8; 4usize],
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub enum MultiSignature {
                                #[codec(index = 0)]
                                Ed25519(runtime_types::sp_core::ed25519::Signature),
                                #[codec(index = 1)]
                                Sr25519(runtime_types::sp_core::sr25519::Signature),
                                #[codec(index = 2)]
                                Ecdsa(runtime_types::sp_core::ecdsa::Signature),
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub enum TokenError {
                                #[codec(index = 0)]
                                FundsUnavailable,
                                #[codec(index = 1)]
                                OnlyProvider,
                                #[codec(index = 2)]
                                BelowMinimum,
                                #[codec(index = 3)]
                                CannotCreate,
                                #[codec(index = 4)]
                                UnknownAsset,
                                #[codec(index = 5)]
                                Frozen,
                                #[codec(index = 6)]
                                Unsupported,
                                #[codec(index = 7)]
                                CannotCreateHold,
                                #[codec(index = 8)]
                                NotExpendable,
                                #[codec(index = 9)]
                                Blocked,
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub enum TransactionalError {
                                #[codec(index = 0)]
                                LimitReached,
                                #[codec(index = 1)]
                                NoLayer,
                        }
                }
                pub mod sp_trie {
                        use super::runtime_types;
                        pub mod storage_proof {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct StorageProof {
                                        pub trie_nodes: ::std::vec::Vec<::std::vec::Vec<::core::primitive::u8>>,
                                }
                        }
                }
                pub mod sp_version {
                        use super::runtime_types;
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub struct RuntimeVersion {
                                pub spec_name: ::std::string::String,
                                pub impl_name: ::std::string::String,
                                pub authoring_version: ::core::primitive::u32,
                                pub spec_version: ::core::primitive::u32,
                                pub impl_version: ::core::primitive::u32,
                                pub apis:
                                        ::std::vec::Vec<([::core::primitive::u8; 8usize], ::core::primitive::u32)>,
                                pub transaction_version: ::core::primitive::u32,
                                pub state_version: ::core::primitive::u8,
                        }
                }
                pub mod sp_weights {
                        use super::runtime_types;
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub struct RuntimeDbWeight {
                                pub read: ::core::primitive::u64,
                                pub write: ::core::primitive::u64,
                        }
                }
                pub mod xcm {
                        use super::runtime_types;
                        pub mod double_encoded {
                                use super::runtime_types;
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct DoubleEncoded {
                                        pub encoded: ::std::vec::Vec<::core::primitive::u8>,
                                }
                        }
                        pub mod v2 {
                                use super::runtime_types;
                                pub mod junction {
                                        use super::runtime_types;
                                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                        pub enum Junction {
                                                #[codec(index = 0)]
                                                Parachain(#[codec(compact)] ::core::primitive::u32),
                                                #[codec(index = 1)]
                                                AccountId32 {
                                                        network: runtime_types::xcm::v2::NetworkId,
                                                        id: [::core::primitive::u8; 32usize],
                                                },
                                                #[codec(index = 2)]
                                                AccountIndex64 {
                                                        network: runtime_types::xcm::v2::NetworkId,
                                                        #[codec(compact)]
                                                        index: ::core::primitive::u64,
                                                },
                                                #[codec(index = 3)]
                                                AccountKey20 {
                                                        network: runtime_types::xcm::v2::NetworkId,
                                                        key: [::core::primitive::u8; 20usize],
                                                },
                                                #[codec(index = 4)]
                                                PalletInstance(::core::primitive::u8),
                                                #[codec(index = 5)]
                                                GeneralIndex(#[codec(compact)] ::core::primitive::u128),
                                                #[codec(index = 6)]
                                                GeneralKey(
                                                        runtime_types::bounded_collections::weak_bounded_vec::WeakBoundedVec<
                                                                ::core::primitive::u8,
                                                        >,
                                                ),
                                                #[codec(index = 7)]
                                                OnlyChild,
                                                #[codec(index = 8)]
                                                Plurality {
                                                        id: runtime_types::xcm::v2::BodyId,
                                                        part: runtime_types::xcm::v2::BodyPart,
                                                },
                                        }
                                }
                                pub mod multiasset {
                                        use super::runtime_types;
                                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                        pub enum AssetId {
                                                #[codec(index = 0)]
                                                Concrete(runtime_types::xcm::v2::multilocation::MultiLocation),
                                                #[codec(index = 1)]
                                                Abstract(::std::vec::Vec<::core::primitive::u8>),
                                        }
                                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                        pub enum AssetInstance {
                                                #[codec(index = 0)]
                                                Undefined,
                                                #[codec(index = 1)]
                                                Index(#[codec(compact)] ::core::primitive::u128),
                                                #[codec(index = 2)]
                                                Array4([::core::primitive::u8; 4usize]),
                                                #[codec(index = 3)]
                                                Array8([::core::primitive::u8; 8usize]),
                                                #[codec(index = 4)]
                                                Array16([::core::primitive::u8; 16usize]),
                                                #[codec(index = 5)]
                                                Array32([::core::primitive::u8; 32usize]),
                                                #[codec(index = 6)]
                                                Blob(::std::vec::Vec<::core::primitive::u8>),
                                        }
                                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                        pub enum Fungibility {
                                                #[codec(index = 0)]
                                                Fungible(#[codec(compact)] ::core::primitive::u128),
                                                #[codec(index = 1)]
                                                NonFungible(runtime_types::xcm::v2::multiasset::AssetInstance),
                                        }
                                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                        pub struct MultiAsset {
                                                pub id: runtime_types::xcm::v2::multiasset::AssetId,
                                                pub fun: runtime_types::xcm::v2::multiasset::Fungibility,
                                        }
                                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                        pub enum MultiAssetFilter {
                                                #[codec(index = 0)]
                                                Definite(runtime_types::xcm::v2::multiasset::MultiAssets),
                                                #[codec(index = 1)]
                                                Wild(runtime_types::xcm::v2::multiasset::WildMultiAsset),
                                        }
                                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                        pub struct MultiAssets(
                                                pub ::std::vec::Vec<runtime_types::xcm::v2::multiasset::MultiAsset>,
                                        );
                                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                        pub enum WildFungibility {
                                                #[codec(index = 0)]
                                                Fungible,
                                                #[codec(index = 1)]
                                                NonFungible,
                                        }
                                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                        pub enum WildMultiAsset {
                                                #[codec(index = 0)]
                                                All,
                                                #[codec(index = 1)]
                                                AllOf {
                                                        id: runtime_types::xcm::v2::multiasset::AssetId,
                                                        fun: runtime_types::xcm::v2::multiasset::WildFungibility,
                                                },
                                        }
                                }
                                pub mod multilocation {
                                        use super::runtime_types;
                                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                        pub enum Junctions {
                                                #[codec(index = 0)]
                                                Here,
                                                #[codec(index = 1)]
                                                X1(runtime_types::xcm::v2::junction::Junction),
                                                #[codec(index = 2)]
                                                X2(
                                                        runtime_types::xcm::v2::junction::Junction,
                                                        runtime_types::xcm::v2::junction::Junction,
                                                ),
                                                #[codec(index = 3)]
                                                X3(
                                                        runtime_types::xcm::v2::junction::Junction,
                                                        runtime_types::xcm::v2::junction::Junction,
                                                        runtime_types::xcm::v2::junction::Junction,
                                                ),
                                                #[codec(index = 4)]
                                                X4(
                                                        runtime_types::xcm::v2::junction::Junction,
                                                        runtime_types::xcm::v2::junction::Junction,
                                                        runtime_types::xcm::v2::junction::Junction,
                                                        runtime_types::xcm::v2::junction::Junction,
                                                ),
                                                #[codec(index = 5)]
                                                X5(
                                                        runtime_types::xcm::v2::junction::Junction,
                                                        runtime_types::xcm::v2::junction::Junction,
                                                        runtime_types::xcm::v2::junction::Junction,
                                                        runtime_types::xcm::v2::junction::Junction,
                                                        runtime_types::xcm::v2::junction::Junction,
                                                ),
                                                #[codec(index = 6)]
                                                X6(
                                                        runtime_types::xcm::v2::junction::Junction,
                                                        runtime_types::xcm::v2::junction::Junction,
                                                        runtime_types::xcm::v2::junction::Junction,
                                                        runtime_types::xcm::v2::junction::Junction,
                                                        runtime_types::xcm::v2::junction::Junction,
                                                        runtime_types::xcm::v2::junction::Junction,
                                                ),
                                                #[codec(index = 7)]
                                                X7(
                                                        runtime_types::xcm::v2::junction::Junction,
                                                        runtime_types::xcm::v2::junction::Junction,
                                                        runtime_types::xcm::v2::junction::Junction,
                                                        runtime_types::xcm::v2::junction::Junction,
                                                        runtime_types::xcm::v2::junction::Junction,
                                                        runtime_types::xcm::v2::junction::Junction,
                                                        runtime_types::xcm::v2::junction::Junction,
                                                ),
                                                #[codec(index = 8)]
                                                X8(
                                                        runtime_types::xcm::v2::junction::Junction,
                                                        runtime_types::xcm::v2::junction::Junction,
                                                        runtime_types::xcm::v2::junction::Junction,
                                                        runtime_types::xcm::v2::junction::Junction,
                                                        runtime_types::xcm::v2::junction::Junction,
                                                        runtime_types::xcm::v2::junction::Junction,
                                                        runtime_types::xcm::v2::junction::Junction,
                                                        runtime_types::xcm::v2::junction::Junction,
                                                ),
                                        }
                                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                        pub struct MultiLocation {
                                                pub parents: ::core::primitive::u8,
                                                pub interior: runtime_types::xcm::v2::multilocation::Junctions,
                                        }
                                }
                                pub mod traits {
                                        use super::runtime_types;
                                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                        pub enum Error {
                                                #[codec(index = 0)]
                                                Overflow,
                                                #[codec(index = 1)]
                                                Unimplemented,
                                                #[codec(index = 2)]
                                                UntrustedReserveLocation,
                                                #[codec(index = 3)]
                                                UntrustedTeleportLocation,
                                                #[codec(index = 4)]
                                                MultiLocationFull,
                                                #[codec(index = 5)]
                                                MultiLocationNotInvertible,
                                                #[codec(index = 6)]
                                                BadOrigin,
                                                #[codec(index = 7)]
                                                InvalidLocation,
                                                #[codec(index = 8)]
                                                AssetNotFound,
                                                #[codec(index = 9)]
                                                FailedToTransactAsset,
                                                #[codec(index = 10)]
                                                NotWithdrawable,
                                                #[codec(index = 11)]
                                                LocationCannotHold,
                                                #[codec(index = 12)]
                                                ExceedsMaxMessageSize,
                                                #[codec(index = 13)]
                                                DestinationUnsupported,
                                                #[codec(index = 14)]
                                                Transport,
                                                #[codec(index = 15)]
                                                Unroutable,
                                                #[codec(index = 16)]
                                                UnknownClaim,
                                                #[codec(index = 17)]
                                                FailedToDecode,
                                                #[codec(index = 18)]
                                                MaxWeightInvalid,
                                                #[codec(index = 19)]
                                                NotHoldingFees,
                                                #[codec(index = 20)]
                                                TooExpensive,
                                                #[codec(index = 21)]
                                                Trap(::core::primitive::u64),
                                                #[codec(index = 22)]
                                                UnhandledXcmVersion,
                                                #[codec(index = 23)]
                                                WeightLimitReached(::core::primitive::u64),
                                                #[codec(index = 24)]
                                                Barrier,
                                                #[codec(index = 25)]
                                                WeightNotComputable,
                                        }
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum BodyId {
                                        #[codec(index = 0)]
                                        Unit,
                                        #[codec(index = 1)]
                                        Named(
                                                runtime_types::bounded_collections::weak_bounded_vec::WeakBoundedVec<
                                                        ::core::primitive::u8,
                                                >,
                                        ),
                                        #[codec(index = 2)]
                                        Index(#[codec(compact)] ::core::primitive::u32),
                                        #[codec(index = 3)]
                                        Executive,
                                        #[codec(index = 4)]
                                        Technical,
                                        #[codec(index = 5)]
                                        Legislative,
                                        #[codec(index = 6)]
                                        Judicial,
                                        #[codec(index = 7)]
                                        Defense,
                                        #[codec(index = 8)]
                                        Administration,
                                        #[codec(index = 9)]
                                        Treasury,
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum BodyPart {
                                        #[codec(index = 0)]
                                        Voice,
                                        #[codec(index = 1)]
                                        Members {
                                                #[codec(compact)]
                                                count: ::core::primitive::u32,
                                        },
                                        #[codec(index = 2)]
                                        Fraction {
                                                #[codec(compact)]
                                                nom: ::core::primitive::u32,
                                                #[codec(compact)]
                                                denom: ::core::primitive::u32,
                                        },
                                        #[codec(index = 3)]
                                        AtLeastProportion {
                                                #[codec(compact)]
                                                nom: ::core::primitive::u32,
                                                #[codec(compact)]
                                                denom: ::core::primitive::u32,
                                        },
                                        #[codec(index = 4)]
                                        MoreThanProportion {
                                                #[codec(compact)]
                                                nom: ::core::primitive::u32,
                                                #[codec(compact)]
                                                denom: ::core::primitive::u32,
                                        },
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Instruction {
                                        #[codec(index = 0)]
                                        WithdrawAsset(runtime_types::xcm::v2::multiasset::MultiAssets),
                                        #[codec(index = 1)]
                                        ReserveAssetDeposited(runtime_types::xcm::v2::multiasset::MultiAssets),
                                        #[codec(index = 2)]
                                        ReceiveTeleportedAsset(runtime_types::xcm::v2::multiasset::MultiAssets),
                                        #[codec(index = 3)]
                                        QueryResponse {
                                                #[codec(compact)]
                                                query_id: ::core::primitive::u64,
                                                response: runtime_types::xcm::v2::Response,
                                                #[codec(compact)]
                                                max_weight: ::core::primitive::u64,
                                        },
                                        #[codec(index = 4)]
                                        TransferAsset {
                                                assets: runtime_types::xcm::v2::multiasset::MultiAssets,
                                                beneficiary: runtime_types::xcm::v2::multilocation::MultiLocation,
                                        },
                                        #[codec(index = 5)]
                                        TransferReserveAsset {
                                                assets: runtime_types::xcm::v2::multiasset::MultiAssets,
                                                dest: runtime_types::xcm::v2::multilocation::MultiLocation,
                                                xcm: runtime_types::xcm::v2::Xcm,
                                        },
                                        #[codec(index = 6)]
                                        Transact {
                                                origin_type: runtime_types::xcm::v2::OriginKind,
                                                #[codec(compact)]
                                                require_weight_at_most: ::core::primitive::u64,
                                                call: runtime_types::xcm::double_encoded::DoubleEncoded,
                                        },
                                        #[codec(index = 7)]
                                        HrmpNewChannelOpenRequest {
                                                #[codec(compact)]
                                                sender: ::core::primitive::u32,
                                                #[codec(compact)]
                                                max_message_size: ::core::primitive::u32,
                                                #[codec(compact)]
                                                max_capacity: ::core::primitive::u32,
                                        },
                                        #[codec(index = 8)]
                                        HrmpChannelAccepted {
                                                #[codec(compact)]
                                                recipient: ::core::primitive::u32,
                                        },
                                        #[codec(index = 9)]
                                        HrmpChannelClosing {
                                                #[codec(compact)]
                                                initiator: ::core::primitive::u32,
                                                #[codec(compact)]
                                                sender: ::core::primitive::u32,
                                                #[codec(compact)]
                                                recipient: ::core::primitive::u32,
                                        },
                                        #[codec(index = 10)]
                                        ClearOrigin,
                                        #[codec(index = 11)]
                                        DescendOrigin(runtime_types::xcm::v2::multilocation::Junctions),
                                        #[codec(index = 12)]
                                        ReportError {
                                                #[codec(compact)]
                                                query_id: ::core::primitive::u64,
                                                dest: runtime_types::xcm::v2::multilocation::MultiLocation,
                                                #[codec(compact)]
                                                max_response_weight: ::core::primitive::u64,
                                        },
                                        #[codec(index = 13)]
                                        DepositAsset {
                                                assets: runtime_types::xcm::v2::multiasset::MultiAssetFilter,
                                                #[codec(compact)]
                                                max_assets: ::core::primitive::u32,
                                                beneficiary: runtime_types::xcm::v2::multilocation::MultiLocation,
                                        },
                                        #[codec(index = 14)]
                                        DepositReserveAsset {
                                                assets: runtime_types::xcm::v2::multiasset::MultiAssetFilter,
                                                #[codec(compact)]
                                                max_assets: ::core::primitive::u32,
                                                dest: runtime_types::xcm::v2::multilocation::MultiLocation,
                                                xcm: runtime_types::xcm::v2::Xcm,
                                        },
                                        #[codec(index = 15)]
                                        ExchangeAsset {
                                                give: runtime_types::xcm::v2::multiasset::MultiAssetFilter,
                                                receive: runtime_types::xcm::v2::multiasset::MultiAssets,
                                        },
                                        #[codec(index = 16)]
                                        InitiateReserveWithdraw {
                                                assets: runtime_types::xcm::v2::multiasset::MultiAssetFilter,
                                                reserve: runtime_types::xcm::v2::multilocation::MultiLocation,
                                                xcm: runtime_types::xcm::v2::Xcm,
                                        },
                                        #[codec(index = 17)]
                                        InitiateTeleport {
                                                assets: runtime_types::xcm::v2::multiasset::MultiAssetFilter,
                                                dest: runtime_types::xcm::v2::multilocation::MultiLocation,
                                                xcm: runtime_types::xcm::v2::Xcm,
                                        },
                                        #[codec(index = 18)]
                                        QueryHolding {
                                                #[codec(compact)]
                                                query_id: ::core::primitive::u64,
                                                dest: runtime_types::xcm::v2::multilocation::MultiLocation,
                                                assets: runtime_types::xcm::v2::multiasset::MultiAssetFilter,
                                                #[codec(compact)]
                                                max_response_weight: ::core::primitive::u64,
                                        },
                                        #[codec(index = 19)]
                                        BuyExecution {
                                                fees: runtime_types::xcm::v2::multiasset::MultiAsset,
                                                weight_limit: runtime_types::xcm::v2::WeightLimit,
                                        },
                                        #[codec(index = 20)]
                                        RefundSurplus,
                                        #[codec(index = 21)]
                                        SetErrorHandler(runtime_types::xcm::v2::Xcm),
                                        #[codec(index = 22)]
                                        SetAppendix(runtime_types::xcm::v2::Xcm),
                                        #[codec(index = 23)]
                                        ClearError,
                                        #[codec(index = 24)]
                                        ClaimAsset {
                                                assets: runtime_types::xcm::v2::multiasset::MultiAssets,
                                                ticket: runtime_types::xcm::v2::multilocation::MultiLocation,
                                        },
                                        #[codec(index = 25)]
                                        Trap(#[codec(compact)] ::core::primitive::u64),
                                        #[codec(index = 26)]
                                        SubscribeVersion {
                                                #[codec(compact)]
                                                query_id: ::core::primitive::u64,
                                                #[codec(compact)]
                                                max_response_weight: ::core::primitive::u64,
                                        },
                                        #[codec(index = 27)]
                                        UnsubscribeVersion,
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum NetworkId {
                                        #[codec(index = 0)]
                                        Any,
                                        #[codec(index = 1)]
                                        Named(
                                                runtime_types::bounded_collections::weak_bounded_vec::WeakBoundedVec<
                                                        ::core::primitive::u8,
                                                >,
                                        ),
                                        #[codec(index = 2)]
                                        Polkadot,
                                        #[codec(index = 3)]
                                        Kusama,
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum OriginKind {
                                        #[codec(index = 0)]
                                        Native,
                                        #[codec(index = 1)]
                                        SovereignAccount,
                                        #[codec(index = 2)]
                                        Superuser,
                                        #[codec(index = 3)]
                                        Xcm,
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Response {
                                        #[codec(index = 0)]
                                        Null,
                                        #[codec(index = 1)]
                                        Assets(runtime_types::xcm::v2::multiasset::MultiAssets),
                                        #[codec(index = 2)]
                                        ExecutionResult(
                                                ::core::option::Option<(
                                                        ::core::primitive::u32,
                                                        runtime_types::xcm::v2::traits::Error,
                                                )>,
                                        ),
                                        #[codec(index = 3)]
                                        Version(::core::primitive::u32),
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum WeightLimit {
                                        #[codec(index = 0)]
                                        Unlimited,
                                        #[codec(index = 1)]
                                        Limited(#[codec(compact)] ::core::primitive::u64),
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct Xcm(pub ::std::vec::Vec<runtime_types::xcm::v2::Instruction>);
                        }
                        pub mod v3 {
                                use super::runtime_types;
                                pub mod junction {
                                        use super::runtime_types;
                                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                        pub enum BodyId {
                                                #[codec(index = 0)]
                                                Unit,
                                                #[codec(index = 1)]
                                                Moniker([::core::primitive::u8; 4usize]),
                                                #[codec(index = 2)]
                                                Index(#[codec(compact)] ::core::primitive::u32),
                                                #[codec(index = 3)]
                                                Executive,
                                                #[codec(index = 4)]
                                                Technical,
                                                #[codec(index = 5)]
                                                Legislative,
                                                #[codec(index = 6)]
                                                Judicial,
                                                #[codec(index = 7)]
                                                Defense,
                                                #[codec(index = 8)]
                                                Administration,
                                                #[codec(index = 9)]
                                                Treasury,
                                        }
                                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                        pub enum BodyPart {
                                                #[codec(index = 0)]
                                                Voice,
                                                #[codec(index = 1)]
                                                Members {
                                                        #[codec(compact)]
                                                        count: ::core::primitive::u32,
                                                },
                                                #[codec(index = 2)]
                                                Fraction {
                                                        #[codec(compact)]
                                                        nom: ::core::primitive::u32,
                                                        #[codec(compact)]
                                                        denom: ::core::primitive::u32,
                                                },
                                                #[codec(index = 3)]
                                                AtLeastProportion {
                                                        #[codec(compact)]
                                                        nom: ::core::primitive::u32,
                                                        #[codec(compact)]
                                                        denom: ::core::primitive::u32,
                                                },
                                                #[codec(index = 4)]
                                                MoreThanProportion {
                                                        #[codec(compact)]
                                                        nom: ::core::primitive::u32,
                                                        #[codec(compact)]
                                                        denom: ::core::primitive::u32,
                                                },
                                        }
                                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                        pub enum Junction {
                                                #[codec(index = 0)]
                                                Parachain(#[codec(compact)] ::core::primitive::u32),
                                                #[codec(index = 1)]
                                                AccountId32 {
                                                        network:
                                                                ::core::option::Option<runtime_types::xcm::v3::junction::NetworkId>,
                                                        id: [::core::primitive::u8; 32usize],
                                                },
                                                #[codec(index = 2)]
                                                AccountIndex64 {
                                                        network:
                                                                ::core::option::Option<runtime_types::xcm::v3::junction::NetworkId>,
                                                        #[codec(compact)]
                                                        index: ::core::primitive::u64,
                                                },
                                                #[codec(index = 3)]
                                                AccountKey20 {
                                                        network:
                                                                ::core::option::Option<runtime_types::xcm::v3::junction::NetworkId>,
                                                        key: [::core::primitive::u8; 20usize],
                                                },
                                                #[codec(index = 4)]
                                                PalletInstance(::core::primitive::u8),
                                                #[codec(index = 5)]
                                                GeneralIndex(#[codec(compact)] ::core::primitive::u128),
                                                #[codec(index = 6)]
                                                GeneralKey {
                                                        length: ::core::primitive::u8,
                                                        data: [::core::primitive::u8; 32usize],
                                                },
                                                #[codec(index = 7)]
                                                OnlyChild,
                                                #[codec(index = 8)]
                                                Plurality {
                                                        id: runtime_types::xcm::v3::junction::BodyId,
                                                        part: runtime_types::xcm::v3::junction::BodyPart,
                                                },
                                                #[codec(index = 9)]
                                                GlobalConsensus(runtime_types::xcm::v3::junction::NetworkId),
                                        }
                                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                        pub enum NetworkId {
                                                #[codec(index = 0)]
                                                ByGenesis([::core::primitive::u8; 32usize]),
                                                #[codec(index = 1)]
                                                ByFork {
                                                        block_number: ::core::primitive::u64,
                                                        block_hash: [::core::primitive::u8; 32usize],
                                                },
                                                #[codec(index = 2)]
                                                Polkadot,
                                                #[codec(index = 3)]
                                                Kusama,
                                                #[codec(index = 4)]
                                                Westend,
                                                #[codec(index = 5)]
                                                Rococo,
                                                #[codec(index = 6)]
                                                Wococo,
                                                #[codec(index = 7)]
                                                Ethereum {
                                                        #[codec(compact)]
                                                        chain_id: ::core::primitive::u64,
                                                },
                                                #[codec(index = 8)]
                                                BitcoinCore,
                                                #[codec(index = 9)]
                                                BitcoinCash,
                                        }
                                }
                                pub mod junctions {
                                        use super::runtime_types;
                                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                        pub enum Junctions {
                                                #[codec(index = 0)]
                                                Here,
                                                #[codec(index = 1)]
                                                X1(runtime_types::xcm::v3::junction::Junction),
                                                #[codec(index = 2)]
                                                X2(
                                                        runtime_types::xcm::v3::junction::Junction,
                                                        runtime_types::xcm::v3::junction::Junction,
                                                ),
                                                #[codec(index = 3)]
                                                X3(
                                                        runtime_types::xcm::v3::junction::Junction,
                                                        runtime_types::xcm::v3::junction::Junction,
                                                        runtime_types::xcm::v3::junction::Junction,
                                                ),
                                                #[codec(index = 4)]
                                                X4(
                                                        runtime_types::xcm::v3::junction::Junction,
                                                        runtime_types::xcm::v3::junction::Junction,
                                                        runtime_types::xcm::v3::junction::Junction,
                                                        runtime_types::xcm::v3::junction::Junction,
                                                ),
                                                #[codec(index = 5)]
                                                X5(
                                                        runtime_types::xcm::v3::junction::Junction,
                                                        runtime_types::xcm::v3::junction::Junction,
                                                        runtime_types::xcm::v3::junction::Junction,
                                                        runtime_types::xcm::v3::junction::Junction,
                                                        runtime_types::xcm::v3::junction::Junction,
                                                ),
                                                #[codec(index = 6)]
                                                X6(
                                                        runtime_types::xcm::v3::junction::Junction,
                                                        runtime_types::xcm::v3::junction::Junction,
                                                        runtime_types::xcm::v3::junction::Junction,
                                                        runtime_types::xcm::v3::junction::Junction,
                                                        runtime_types::xcm::v3::junction::Junction,
                                                        runtime_types::xcm::v3::junction::Junction,
                                                ),
                                                #[codec(index = 7)]
                                                X7(
                                                        runtime_types::xcm::v3::junction::Junction,
                                                        runtime_types::xcm::v3::junction::Junction,
                                                        runtime_types::xcm::v3::junction::Junction,
                                                        runtime_types::xcm::v3::junction::Junction,
                                                        runtime_types::xcm::v3::junction::Junction,
                                                        runtime_types::xcm::v3::junction::Junction,
                                                        runtime_types::xcm::v3::junction::Junction,
                                                ),
                                                #[codec(index = 8)]
                                                X8(
                                                        runtime_types::xcm::v3::junction::Junction,
                                                        runtime_types::xcm::v3::junction::Junction,
                                                        runtime_types::xcm::v3::junction::Junction,
                                                        runtime_types::xcm::v3::junction::Junction,
                                                        runtime_types::xcm::v3::junction::Junction,
                                                        runtime_types::xcm::v3::junction::Junction,
                                                        runtime_types::xcm::v3::junction::Junction,
                                                        runtime_types::xcm::v3::junction::Junction,
                                                ),
                                        }
                                }
                                pub mod multiasset {
                                        use super::runtime_types;
                                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                        pub enum AssetId {
                                                #[codec(index = 0)]
                                                Concrete(runtime_types::xcm::v3::multilocation::MultiLocation),
                                                #[codec(index = 1)]
                                                Abstract([::core::primitive::u8; 32usize]),
                                        }
                                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                        pub enum AssetInstance {
                                                #[codec(index = 0)]
                                                Undefined,
                                                #[codec(index = 1)]
                                                Index(#[codec(compact)] ::core::primitive::u128),
                                                #[codec(index = 2)]
                                                Array4([::core::primitive::u8; 4usize]),
                                                #[codec(index = 3)]
                                                Array8([::core::primitive::u8; 8usize]),
                                                #[codec(index = 4)]
                                                Array16([::core::primitive::u8; 16usize]),
                                                #[codec(index = 5)]
                                                Array32([::core::primitive::u8; 32usize]),
                                        }
                                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                        pub enum Fungibility {
                                                #[codec(index = 0)]
                                                Fungible(#[codec(compact)] ::core::primitive::u128),
                                                #[codec(index = 1)]
                                                NonFungible(runtime_types::xcm::v3::multiasset::AssetInstance),
                                        }
                                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                        pub struct MultiAsset {
                                                pub id: runtime_types::xcm::v3::multiasset::AssetId,
                                                pub fun: runtime_types::xcm::v3::multiasset::Fungibility,
                                        }
                                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                        pub enum MultiAssetFilter {
                                                #[codec(index = 0)]
                                                Definite(runtime_types::xcm::v3::multiasset::MultiAssets),
                                                #[codec(index = 1)]
                                                Wild(runtime_types::xcm::v3::multiasset::WildMultiAsset),
                                        }
                                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                        pub struct MultiAssets(
                                                pub ::std::vec::Vec<runtime_types::xcm::v3::multiasset::MultiAsset>,
                                        );
                                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                        pub enum WildFungibility {
                                                #[codec(index = 0)]
                                                Fungible,
                                                #[codec(index = 1)]
                                                NonFungible,
                                        }
                                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                        pub enum WildMultiAsset {
                                                #[codec(index = 0)]
                                                All,
                                                #[codec(index = 1)]
                                                AllOf {
                                                        id: runtime_types::xcm::v3::multiasset::AssetId,
                                                        fun: runtime_types::xcm::v3::multiasset::WildFungibility,
                                                },
                                                #[codec(index = 2)]
                                                AllCounted(#[codec(compact)] ::core::primitive::u32),
                                                #[codec(index = 3)]
                                                AllOfCounted {
                                                        id: runtime_types::xcm::v3::multiasset::AssetId,
                                                        fun: runtime_types::xcm::v3::multiasset::WildFungibility,
                                                        #[codec(compact)]
                                                        count: ::core::primitive::u32,
                                                },
                                        }
                                }
                                pub mod multilocation {
                                        use super::runtime_types;
                                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                        pub struct MultiLocation {
                                                pub parents: ::core::primitive::u8,
                                                pub interior: runtime_types::xcm::v3::junctions::Junctions,
                                        }
                                }
                                pub mod traits {
                                        use super::runtime_types;
                                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                        pub enum Error {
                                                #[codec(index = 0)]
                                                Overflow,
                                                #[codec(index = 1)]
                                                Unimplemented,
                                                #[codec(index = 2)]
                                                UntrustedReserveLocation,
                                                #[codec(index = 3)]
                                                UntrustedTeleportLocation,
                                                #[codec(index = 4)]
                                                LocationFull,
                                                #[codec(index = 5)]
                                                LocationNotInvertible,
                                                #[codec(index = 6)]
                                                BadOrigin,
                                                #[codec(index = 7)]
                                                InvalidLocation,
                                                #[codec(index = 8)]
                                                AssetNotFound,
                                                #[codec(index = 9)]
                                                FailedToTransactAsset,
                                                #[codec(index = 10)]
                                                NotWithdrawable,
                                                #[codec(index = 11)]
                                                LocationCannotHold,
                                                #[codec(index = 12)]
                                                ExceedsMaxMessageSize,
                                                #[codec(index = 13)]
                                                DestinationUnsupported,
                                                #[codec(index = 14)]
                                                Transport,
                                                #[codec(index = 15)]
                                                Unroutable,
                                                #[codec(index = 16)]
                                                UnknownClaim,
                                                #[codec(index = 17)]
                                                FailedToDecode,
                                                #[codec(index = 18)]
                                                MaxWeightInvalid,
                                                #[codec(index = 19)]
                                                NotHoldingFees,
                                                #[codec(index = 20)]
                                                TooExpensive,
                                                #[codec(index = 21)]
                                                Trap(::core::primitive::u64),
                                                #[codec(index = 22)]
                                                ExpectationFalse,
                                                #[codec(index = 23)]
                                                PalletNotFound,
                                                #[codec(index = 24)]
                                                NameMismatch,
                                                #[codec(index = 25)]
                                                VersionIncompatible,
                                                #[codec(index = 26)]
                                                HoldingWouldOverflow,
                                                #[codec(index = 27)]
                                                ExportError,
                                                #[codec(index = 28)]
                                                ReanchorFailed,
                                                #[codec(index = 29)]
                                                NoDeal,
                                                #[codec(index = 30)]
                                                FeesNotMet,
                                                #[codec(index = 31)]
                                                LockError,
                                                #[codec(index = 32)]
                                                NoPermission,
                                                #[codec(index = 33)]
                                                Unanchored,
                                                #[codec(index = 34)]
                                                NotDepositable,
                                                #[codec(index = 35)]
                                                UnhandledXcmVersion,
                                                #[codec(index = 36)]
                                                WeightLimitReached(::sp_weights::Weight),
                                                #[codec(index = 37)]
                                                Barrier,
                                                #[codec(index = 38)]
                                                WeightNotComputable,
                                                #[codec(index = 39)]
                                                ExceedsStackLimit,
                                        }
                                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                        pub enum Outcome {
                                                #[codec(index = 0)]
                                                Complete(::sp_weights::Weight),
                                                #[codec(index = 1)]
                                                Incomplete(::sp_weights::Weight, runtime_types::xcm::v3::traits::Error),
                                                #[codec(index = 2)]
                                                Error(runtime_types::xcm::v3::traits::Error),
                                        }
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Instruction {
                                        #[codec(index = 0)]
                                        WithdrawAsset(runtime_types::xcm::v3::multiasset::MultiAssets),
                                        #[codec(index = 1)]
                                        ReserveAssetDeposited(runtime_types::xcm::v3::multiasset::MultiAssets),
                                        #[codec(index = 2)]
                                        ReceiveTeleportedAsset(runtime_types::xcm::v3::multiasset::MultiAssets),
                                        #[codec(index = 3)]
                                        QueryResponse {
                                                #[codec(compact)]
                                                query_id: ::core::primitive::u64,
                                                response: runtime_types::xcm::v3::Response,
                                                max_weight: ::sp_weights::Weight,
                                                querier: ::core::option::Option<
                                                        runtime_types::xcm::v3::multilocation::MultiLocation,
                                                >,
                                        },
                                        #[codec(index = 4)]
                                        TransferAsset {
                                                assets: runtime_types::xcm::v3::multiasset::MultiAssets,
                                                beneficiary: runtime_types::xcm::v3::multilocation::MultiLocation,
                                        },
                                        #[codec(index = 5)]
                                        TransferReserveAsset {
                                                assets: runtime_types::xcm::v3::multiasset::MultiAssets,
                                                dest: runtime_types::xcm::v3::multilocation::MultiLocation,
                                                xcm: runtime_types::xcm::v3::Xcm,
                                        },
                                        #[codec(index = 6)]
                                        Transact {
                                                origin_kind: runtime_types::xcm::v2::OriginKind,
                                                require_weight_at_most: ::sp_weights::Weight,
                                                call: runtime_types::xcm::double_encoded::DoubleEncoded,
                                        },
                                        #[codec(index = 7)]
                                        HrmpNewChannelOpenRequest {
                                                #[codec(compact)]
                                                sender: ::core::primitive::u32,
                                                #[codec(compact)]
                                                max_message_size: ::core::primitive::u32,
                                                #[codec(compact)]
                                                max_capacity: ::core::primitive::u32,
                                        },
                                        #[codec(index = 8)]
                                        HrmpChannelAccepted {
                                                #[codec(compact)]
                                                recipient: ::core::primitive::u32,
                                        },
                                        #[codec(index = 9)]
                                        HrmpChannelClosing {
                                                #[codec(compact)]
                                                initiator: ::core::primitive::u32,
                                                #[codec(compact)]
                                                sender: ::core::primitive::u32,
                                                #[codec(compact)]
                                                recipient: ::core::primitive::u32,
                                        },
                                        #[codec(index = 10)]
                                        ClearOrigin,
                                        #[codec(index = 11)]
                                        DescendOrigin(runtime_types::xcm::v3::junctions::Junctions),
                                        #[codec(index = 12)]
                                        ReportError(runtime_types::xcm::v3::QueryResponseInfo),
                                        #[codec(index = 13)]
                                        DepositAsset {
                                                assets: runtime_types::xcm::v3::multiasset::MultiAssetFilter,
                                                beneficiary: runtime_types::xcm::v3::multilocation::MultiLocation,
                                        },
                                        #[codec(index = 14)]
                                        DepositReserveAsset {
                                                assets: runtime_types::xcm::v3::multiasset::MultiAssetFilter,
                                                dest: runtime_types::xcm::v3::multilocation::MultiLocation,
                                                xcm: runtime_types::xcm::v3::Xcm,
                                        },
                                        #[codec(index = 15)]
                                        ExchangeAsset {
                                                give: runtime_types::xcm::v3::multiasset::MultiAssetFilter,
                                                want: runtime_types::xcm::v3::multiasset::MultiAssets,
                                                maximal: ::core::primitive::bool,
                                        },
                                        #[codec(index = 16)]
                                        InitiateReserveWithdraw {
                                                assets: runtime_types::xcm::v3::multiasset::MultiAssetFilter,
                                                reserve: runtime_types::xcm::v3::multilocation::MultiLocation,
                                                xcm: runtime_types::xcm::v3::Xcm,
                                        },
                                        #[codec(index = 17)]
                                        InitiateTeleport {
                                                assets: runtime_types::xcm::v3::multiasset::MultiAssetFilter,
                                                dest: runtime_types::xcm::v3::multilocation::MultiLocation,
                                                xcm: runtime_types::xcm::v3::Xcm,
                                        },
                                        #[codec(index = 18)]
                                        ReportHolding {
                                                response_info: runtime_types::xcm::v3::QueryResponseInfo,
                                                assets: runtime_types::xcm::v3::multiasset::MultiAssetFilter,
                                        },
                                        #[codec(index = 19)]
                                        BuyExecution {
                                                fees: runtime_types::xcm::v3::multiasset::MultiAsset,
                                                weight_limit: runtime_types::xcm::v3::WeightLimit,
                                        },
                                        #[codec(index = 20)]
                                        RefundSurplus,
                                        #[codec(index = 21)]
                                        SetErrorHandler(runtime_types::xcm::v3::Xcm),
                                        #[codec(index = 22)]
                                        SetAppendix(runtime_types::xcm::v3::Xcm),
                                        #[codec(index = 23)]
                                        ClearError,
                                        #[codec(index = 24)]
                                        ClaimAsset {
                                                assets: runtime_types::xcm::v3::multiasset::MultiAssets,
                                                ticket: runtime_types::xcm::v3::multilocation::MultiLocation,
                                        },
                                        #[codec(index = 25)]
                                        Trap(#[codec(compact)] ::core::primitive::u64),
                                        #[codec(index = 26)]
                                        SubscribeVersion {
                                                #[codec(compact)]
                                                query_id: ::core::primitive::u64,
                                                max_response_weight: ::sp_weights::Weight,
                                        },
                                        #[codec(index = 27)]
                                        UnsubscribeVersion,
                                        #[codec(index = 28)]
                                        BurnAsset(runtime_types::xcm::v3::multiasset::MultiAssets),
                                        #[codec(index = 29)]
                                        ExpectAsset(runtime_types::xcm::v3::multiasset::MultiAssets),
                                        #[codec(index = 30)]
                                        ExpectOrigin(
                                                ::core::option::Option<
                                                        runtime_types::xcm::v3::multilocation::MultiLocation,
                                                >,
                                        ),
                                        #[codec(index = 31)]
                                        ExpectError(
                                                ::core::option::Option<(
                                                        ::core::primitive::u32,
                                                        runtime_types::xcm::v3::traits::Error,
                                                )>,
                                        ),
                                        #[codec(index = 32)]
                                        ExpectTransactStatus(runtime_types::xcm::v3::MaybeErrorCode),
                                        #[codec(index = 33)]
                                        QueryPallet {
                                                module_name: ::std::vec::Vec<::core::primitive::u8>,
                                                response_info: runtime_types::xcm::v3::QueryResponseInfo,
                                        },
                                        #[codec(index = 34)]
                                        ExpectPallet {
                                                #[codec(compact)]
                                                index: ::core::primitive::u32,
                                                name: ::std::vec::Vec<::core::primitive::u8>,
                                                module_name: ::std::vec::Vec<::core::primitive::u8>,
                                                #[codec(compact)]
                                                crate_major: ::core::primitive::u32,
                                                #[codec(compact)]
                                                min_crate_minor: ::core::primitive::u32,
                                        },
                                        #[codec(index = 35)]
                                        ReportTransactStatus(runtime_types::xcm::v3::QueryResponseInfo),
                                        #[codec(index = 36)]
                                        ClearTransactStatus,
                                        #[codec(index = 37)]
                                        UniversalOrigin(runtime_types::xcm::v3::junction::Junction),
                                        #[codec(index = 38)]
                                        ExportMessage {
                                                network: runtime_types::xcm::v3::junction::NetworkId,
                                                destination: runtime_types::xcm::v3::junctions::Junctions,
                                                xcm: runtime_types::xcm::v3::Xcm,
                                        },
                                        #[codec(index = 39)]
                                        LockAsset {
                                                asset: runtime_types::xcm::v3::multiasset::MultiAsset,
                                                unlocker: runtime_types::xcm::v3::multilocation::MultiLocation,
                                        },
                                        #[codec(index = 40)]
                                        UnlockAsset {
                                                asset: runtime_types::xcm::v3::multiasset::MultiAsset,
                                                target: runtime_types::xcm::v3::multilocation::MultiLocation,
                                        },
                                        #[codec(index = 41)]
                                        NoteUnlockable {
                                                asset: runtime_types::xcm::v3::multiasset::MultiAsset,
                                                owner: runtime_types::xcm::v3::multilocation::MultiLocation,
                                        },
                                        #[codec(index = 42)]
                                        RequestUnlock {
                                                asset: runtime_types::xcm::v3::multiasset::MultiAsset,
                                                locker: runtime_types::xcm::v3::multilocation::MultiLocation,
                                        },
                                        #[codec(index = 43)]
                                        SetFeesMode { jit_withdraw: ::core::primitive::bool },
                                        #[codec(index = 44)]
                                        SetTopic([::core::primitive::u8; 32usize]),
                                        #[codec(index = 45)]
                                        ClearTopic,
                                        #[codec(index = 46)]
                                        AliasOrigin(runtime_types::xcm::v3::multilocation::MultiLocation),
                                        #[codec(index = 47)]
                                        UnpaidExecution {
                                                weight_limit: runtime_types::xcm::v3::WeightLimit,
                                                check_origin: ::core::option::Option<
                                                        runtime_types::xcm::v3::multilocation::MultiLocation,
                                                >,
                                        },
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum MaybeErrorCode {
                                        #[codec(index = 0)]
                                        Success,
                                        #[codec(index = 1)]
                                        Error(
                                                runtime_types::bounded_collections::bounded_vec::BoundedVec<
                                                        ::core::primitive::u8,
                                                >,
                                        ),
                                        #[codec(index = 2)]
                                        TruncatedError(
                                                runtime_types::bounded_collections::bounded_vec::BoundedVec<
                                                        ::core::primitive::u8,
                                                >,
                                        ),
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct PalletInfo {
                                        #[codec(compact)]
                                        pub index: ::core::primitive::u32,
                                        pub name: runtime_types::bounded_collections::bounded_vec::BoundedVec<
                                                ::core::primitive::u8,
                                        >,
                                        pub module_name: runtime_types::bounded_collections::bounded_vec::BoundedVec<
                                                ::core::primitive::u8,
                                        >,
                                        #[codec(compact)]
                                        pub major: ::core::primitive::u32,
                                        #[codec(compact)]
                                        pub minor: ::core::primitive::u32,
                                        #[codec(compact)]
                                        pub patch: ::core::primitive::u32,
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct QueryResponseInfo {
                                        pub destination: runtime_types::xcm::v3::multilocation::MultiLocation,
                                        #[codec(compact)]
                                        pub query_id: ::core::primitive::u64,
                                        pub max_weight: ::sp_weights::Weight,
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum Response {
                                        #[codec(index = 0)]
                                        Null,
                                        #[codec(index = 1)]
                                        Assets(runtime_types::xcm::v3::multiasset::MultiAssets),
                                        #[codec(index = 2)]
                                        ExecutionResult(
                                                ::core::option::Option<(
                                                        ::core::primitive::u32,
                                                        runtime_types::xcm::v3::traits::Error,
                                                )>,
                                        ),
                                        #[codec(index = 3)]
                                        Version(::core::primitive::u32),
                                        #[codec(index = 4)]
                                        PalletsInfo(
                                                runtime_types::bounded_collections::bounded_vec::BoundedVec<
                                                        runtime_types::xcm::v3::PalletInfo,
                                                >,
                                        ),
                                        #[codec(index = 5)]
                                        DispatchResult(runtime_types::xcm::v3::MaybeErrorCode),
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub enum WeightLimit {
                                        #[codec(index = 0)]
                                        Unlimited,
                                        #[codec(index = 1)]
                                        Limited(::sp_weights::Weight),
                                }
                                #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                                pub struct Xcm(pub ::std::vec::Vec<runtime_types::xcm::v3::Instruction>);
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub enum VersionedAssetId {
                                #[codec(index = 3)]
                                V3(runtime_types::xcm::v3::multiasset::AssetId),
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub enum VersionedMultiAssets {
                                #[codec(index = 1)]
                                V2(runtime_types::xcm::v2::multiasset::MultiAssets),
                                #[codec(index = 3)]
                                V3(runtime_types::xcm::v3::multiasset::MultiAssets),
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub enum VersionedMultiLocation {
                                #[codec(index = 1)]
                                V2(runtime_types::xcm::v2::multilocation::MultiLocation),
                                #[codec(index = 3)]
                                V3(runtime_types::xcm::v3::multilocation::MultiLocation),
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub enum VersionedResponse {
                                #[codec(index = 2)]
                                V2(runtime_types::xcm::v2::Response),
                                #[codec(index = 3)]
                                V3(runtime_types::xcm::v3::Response),
                        }
                        #[derive(:: codec :: Decode, :: codec :: Encode, Clone, Debug, PartialEq)]
                        pub enum VersionedXcm {
                                #[codec(index = 2)]
                                V2(runtime_types::xcm::v2::Xcm),
                                #[codec(index = 3)]
                                V3(runtime_types::xcm::v3::Xcm),
                        }
                }
        }
}
