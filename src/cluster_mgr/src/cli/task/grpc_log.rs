// Include the auto-generated gRPC code for the LogService
pub mod log_proto {
    tonic::include_proto!("txlog");
}

// Re-export the types needed by other modules
use log_proto::log_service_client::LogServiceClient;
pub use log_proto::{AddPeerRequest, ChangePeersResponse, RemovePeerRequest};
use tonic::{Request, Status};

/// Add a log peer via gRPC.
pub async fn add_log_peer(
    address: &str,
    request: AddPeerRequest,
) -> Result<ChangePeersResponse, Status> {
    // Connect to the log service endpoint
    let mut client = LogServiceClient::connect(address.to_string())
        .await
        .map_err(|e| Status::internal(format!("failed to connect to log service: {}", e)))?;
    // Perform the AddPeer RPC
    let response = client.add_peer(Request::new(request)).await?;
    Ok(response.into_inner())
}

/// Remove a log peer via gRPC.
pub async fn remove_log_peer(
    address: &str,
    request: RemovePeerRequest,
) -> Result<ChangePeersResponse, Status> {
    // Connect to the log service endpoint
    let mut client = LogServiceClient::connect(address.to_string())
        .await
        .map_err(|e| Status::internal(format!("failed to connect to log service: {}", e)))?;
    // Perform the RemovePeer RPC
    let response = client.remove_peer(Request::new(request)).await?;
    Ok(response.into_inner())
}
