use serde::{Deserialize, Serialize};

pub const CURRENT_PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RequestEnvelope {
    pub protocol_version: u32,
    pub request_id: u64,
    pub command: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResponseEnvelope {
    pub protocol_version: u32,
    pub request_id: u64,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<BridgeErrorPayload>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct BridgeErrorPayload {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HandshakeRequest {
    pub client_version: String,
    pub supported_protocol: u32,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HandshakeResponse {
    pub server_version: String,
    pub protocol_version: u32,
    pub target_triple: String,
    pub features: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum SerializedTy {
    Int,
    Float,
    Bool,
    String,
    Void,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum SerializedExpr {
    IntLit(i64),
    StrLit(String),
    BoolLit(bool),
    Var(String),
    Add(Box<SerializedExpr>, Box<SerializedExpr>),
    Call {
        callee: String,
        args: Vec<SerializedExpr>,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum SerializedStmt {
    Let {
        name: String,
        ty: SerializedTy,
        value: SerializedExpr,
    },
    Return(Option<SerializedExpr>),
    Expr(SerializedExpr),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SerializedParam {
    pub name: String,
    pub ty: SerializedTy,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SerializedFunc {
    pub name: String,
    pub params: Vec<SerializedParam>,
    pub return_ty: SerializedTy,
    pub body_stmts: Vec<SerializedStmt>,
    pub is_public: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SerializedModule {
    pub namespace: String,
    pub funcs: Vec<SerializedFunc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CompileModuleRequest {
    pub module: SerializedModule,
    pub output_path: String,
    pub lib_mode: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CompileModuleResponse {
    pub output_path: String,
    pub bytes_written: u64,
}
