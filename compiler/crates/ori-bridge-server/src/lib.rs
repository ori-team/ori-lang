pub mod framing;
pub mod protocol;
pub mod server;

pub use framing::{read_frame, write_frame, FrameError, MAX_FRAME_SIZE, PROTOCOL_MAGIC};
pub use protocol::{
    BridgeErrorPayload, CompileModuleRequest, CompileModuleResponse, HandshakeRequest,
    HandshakeResponse, RequestEnvelope, ResponseEnvelope, SerializedBinaryOp, SerializedExpr,
    SerializedFunc, SerializedModule, SerializedParam, SerializedStmt, SerializedTy,
    CURRENT_PROTOCOL_VERSION,
};
pub use server::BridgeServer;
