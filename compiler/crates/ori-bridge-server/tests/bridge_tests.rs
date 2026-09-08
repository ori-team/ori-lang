use ori_bridge_server::{
    read_frame, write_frame, BridgeServer, HandshakeRequest, RequestEnvelope, CURRENT_PROTOCOL_VERSION,
};
use std::io::Cursor;

#[test]
fn test_framing_round_trip() {
    let payload = b"{\"command\":\"handshake\"}";
    let mut buffer = Vec::new();

    write_frame(&mut buffer, payload).expect("write frame failed");

    let mut cursor = Cursor::new(buffer);
    let received = read_frame(&mut cursor).expect("read frame failed");

    assert_eq!(received, payload);
}

#[test]
fn test_bridge_handshake_flow() {
    let server = BridgeServer::new();

    let handshake_req = HandshakeRequest {
        client_version: "0.3.8".to_string(),
        supported_protocol: 1,
    };

    let req = RequestEnvelope {
        protocol_version: CURRENT_PROTOCOL_VERSION,
        request_id: 1001,
        command: "handshake".to_string(),
        payload: serde_json::to_value(handshake_req).unwrap(),
    };

    let res = server.handle_request(req);
    assert_eq!(res.status, "ok");
    assert_eq!(res.request_id, 1001);
    assert!(res.data.is_some());
    assert!(res.error.is_none());
}

#[test]
fn test_bridge_rejects_incompatible_protocol_version() {
    let server = BridgeServer::new();

    let req = RequestEnvelope {
        protocol_version: 999, // Incompatible version
        request_id: 1002,
        command: "handshake".to_string(),
        payload: serde_json::json!({}),
    };

    let res = server.handle_request(req);
    assert_eq!(res.status, "error");
    let err = res.error.expect("expected error payload");
    assert_eq!(err.code, "bridge.unsupported_version");
}
