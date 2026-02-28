#![no_main]

use arbitrary::Arbitrary;
use emissary_core::{
    Block, Datagram, HeaderReader, Message, MessageBlock, MessageType, Packet, SamCommand,
};
use emissary_util::runtime::tokio::Runtime;
use libfuzzer_sys::fuzz_target;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Arbitrary)]
pub enum GeneratedMessageType {
    BandwidthLimits,
    BlindingInfo,
    CreateLeaseSet,
    CreateLeaseSet2,
    CreateSession,
    DestLookup,
    DestReply,
    DestroySession,
    Disconnect,
    GetBandwidthLimits,
    GetDate,
    HostLookup,
    HostReply,
    MessagePayload,
    MessageStatus,
    ReceiveMessageBegin,
    ReceiveMessageEnd,
    ReconfigureSession,
    ReportAbuse,
    RequestLeaseSet,
    RequestVariableLeaseSet,
    SendMessage,
    SendMessageExpires,
    SessionStatus,
    SetDate,
}

impl Into<MessageType> for GeneratedMessageType {
    fn into(self) -> MessageType {
        match self {
            GeneratedMessageType::BandwidthLimits => MessageType::BandwidthLimits,
            GeneratedMessageType::BlindingInfo => MessageType::BlindingInfo,
            GeneratedMessageType::CreateLeaseSet => MessageType::CreateLeaseSet,
            GeneratedMessageType::CreateLeaseSet2 => MessageType::CreateLeaseSet2,
            GeneratedMessageType::CreateSession => MessageType::CreateSession,
            GeneratedMessageType::DestLookup => MessageType::DestLookup,
            GeneratedMessageType::DestReply => MessageType::DestReply,
            GeneratedMessageType::DestroySession => MessageType::DestroySession,
            GeneratedMessageType::Disconnect => MessageType::Disconnect,
            GeneratedMessageType::GetBandwidthLimits => MessageType::GetBandwidthLimits,
            GeneratedMessageType::GetDate => MessageType::GetDate,
            GeneratedMessageType::HostLookup => MessageType::HostLookup,
            GeneratedMessageType::HostReply => MessageType::HostReply,
            GeneratedMessageType::MessagePayload => MessageType::MessagePayload,
            GeneratedMessageType::MessageStatus => MessageType::MessageStatus,
            GeneratedMessageType::ReceiveMessageBegin => MessageType::ReceiveMessageBegin,
            GeneratedMessageType::ReceiveMessageEnd => MessageType::ReceiveMessageEnd,
            GeneratedMessageType::ReconfigureSession => MessageType::ReconfigureSession,
            GeneratedMessageType::ReportAbuse => MessageType::ReportAbuse,
            GeneratedMessageType::RequestLeaseSet => MessageType::RequestLeaseSet,
            GeneratedMessageType::RequestVariableLeaseSet => MessageType::RequestVariableLeaseSet,
            GeneratedMessageType::SendMessage => MessageType::SendMessage,
            GeneratedMessageType::SendMessageExpires => MessageType::SendMessageExpires,
            GeneratedMessageType::SessionStatus => MessageType::SessionStatus,
            GeneratedMessageType::SetDate => MessageType::SetDate,
        }
    }
}

fuzz_target!(|input: (&[u8], &str, [u8; 32], GeneratedMessageType)| {
    let _ = Block::parse(&input.0);
    let _ = SamCommand::parse::<Runtime>(&input.1);
    let _ = Datagram::parse(&input.0);
    let _ = Message::parse::<Runtime>(input.3.into(), &input.0);
    let _ = MessageBlock::parse(&input.0);
    let _ = Packet::parse::<Runtime>(&input.0);

    let mut bytes = input.0.to_vec();
    if let Ok(mut reader) = HeaderReader::new(input.2, &mut bytes) {
        let _id = reader.dst_id();
        let _ = reader.parse(input.2);
    }
});
