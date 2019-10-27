use crate::constants::VoiceOpCode;
use serde_json::{json, Value};
use super::connection_info::ConnectionInfo;

#[inline]
pub fn build_identify(info: &ConnectionInfo) -> Value {
    json!({
        "op": VoiceOpCode::Identify,
        "d": {
            "server_id": info.guild_id.0,
            "session_id": &info.session_id,
            "token": &info.token,
            "user_id": info.user_id.0,
        }
    })
}

#[inline]
pub fn build_heartbeat(nonce: u64) -> Value {
    json!({
        "op": VoiceOpCode::Heartbeat,
        "d": nonce,
    })
}

#[inline]
pub fn build_resume(info: &ConnectionInfo) -> Value {
    json!({
        "op": VoiceOpCode::Resume,
        "d": {
            "server_id": info.guild_id.0,
            "session_id": &info.session_id,
            "token": &info.token,
        },
    })
}

#[inline]
pub fn build_select_protocol(address: ::std::borrow::Cow<'_, str>, port: u16) -> Value {
    json!({
        "op": VoiceOpCode::SelectProtocol,
        "d": {
            "protocol": "udp",
            "data": {
                "address": address,
                "mode": super::CRYPTO_MODE,
                "port": port,
            }
        }
    })
}

#[inline]
pub fn build_speaking(speaking: bool) -> Value {
    json!({
        "op": VoiceOpCode::Speaking,
        "d": {
            "delay": 0,
            "speaking": speaking,
        }
    })
}
