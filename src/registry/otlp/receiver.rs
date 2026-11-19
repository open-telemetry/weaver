// SPDX-License-Identifier: Apache-2.0

//! The OTLP/gRPC services and the OTLP/HTTP endpoints, as axum routers that
//! queue exports for the checker.

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::header::{CONTENT_ENCODING, CONTENT_TYPE};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::post;
use axum::Router;
use prost::Message;
use tonic::service::Routes;
use tonic::{Code, Request, Response, Status};

use super::admin::AppState;
use super::grpc_stubs::proto::collector::logs::v1::logs_service_server::{
    LogsService, LogsServiceServer,
};
use super::grpc_stubs::proto::collector::logs::v1::{
    ExportLogsServiceRequest, ExportLogsServiceResponse,
};
use super::grpc_stubs::proto::collector::metrics::v1::metrics_service_server::{
    MetricsService, MetricsServiceServer,
};
use super::grpc_stubs::proto::collector::metrics::v1::{
    ExportMetricsServiceRequest, ExportMetricsServiceResponse,
};
use super::grpc_stubs::proto::collector::profiles::v1development::profiles_service_server::{
    ProfilesService, ProfilesServiceServer,
};
use super::grpc_stubs::proto::collector::profiles::v1development::{
    ExportProfilesServiceRequest, ExportProfilesServiceResponse,
};
use super::grpc_stubs::proto::collector::trace::v1::trace_service_server::{
    TraceService, TraceServiceServer,
};
use super::grpc_stubs::proto::collector::trace::v1::{
    ExportTraceServiceRequest, ExportTraceServiceResponse,
};
use super::OtlpRequest;

/// Queues OTLP exports for the checker.
///
/// An export is acknowledged only after it is queued, so an exporter that waits
/// for the acknowledgement knows its data is ahead of any later `Stop`.
#[derive(Clone)]
pub struct OtlpReceiver(Arc<AppState>);

impl OtlpReceiver {
    pub fn new(state: Arc<AppState>) -> Self {
        Self(state)
    }

    /// The four OTLP/gRPC services as an axum router, ready to merge or serve.
    pub fn grpc_router(&self) -> Router {
        Routes::new(LogsServiceServer::new(self.clone()))
            .add_service(MetricsServiceServer::new(self.clone()))
            .add_service(TraceServiceServer::new(self.clone()))
            .add_service(ProfilesServiceServer::new(self.clone()))
            .into_axum_router()
    }

    /// The OTLP/HTTP endpoints as an axum router. They accept binary protobuf.
    pub fn http_router(&self) -> Router {
        Router::new()
            .route("/v1/logs", post(export_http::<ExportLogsServiceRequest>))
            .route(
                "/v1/metrics",
                post(export_http::<ExportMetricsServiceRequest>),
            )
            .route("/v1/traces", post(export_http::<ExportTraceServiceRequest>))
            .route(
                "/v1development/profiles",
                post(export_http::<ExportProfilesServiceRequest>),
            )
            .layer(DefaultBodyLimit::max(MAX_HTTP_BODY_BYTES))
            .with_state(self.clone())
    }

    /// Queues one export. Fails once the checker has stopped reading.
    async fn forward(&self, request: OtlpRequest) -> Result<(), Status> {
        self.0.touch();
        self.0
            .exports
            .send(request)
            .await
            .map_err(|_| Status::unavailable("the OTLP receiver has stopped"))
    }
}

#[tonic::async_trait]
impl LogsService for OtlpReceiver {
    async fn export(
        &self,
        request: Request<ExportLogsServiceRequest>,
    ) -> Result<Response<ExportLogsServiceResponse>, Status> {
        self.forward(OtlpRequest::Logs(request.into_inner()))
            .await?;
        Ok(Response::new(ExportLogsServiceResponse {
            partial_success: None,
        }))
    }
}

#[tonic::async_trait]
impl MetricsService for OtlpReceiver {
    async fn export(
        &self,
        request: Request<ExportMetricsServiceRequest>,
    ) -> Result<Response<ExportMetricsServiceResponse>, Status> {
        self.forward(OtlpRequest::Metrics(request.into_inner()))
            .await?;
        Ok(Response::new(ExportMetricsServiceResponse {
            partial_success: None,
        }))
    }
}

#[tonic::async_trait]
impl TraceService for OtlpReceiver {
    async fn export(
        &self,
        request: Request<ExportTraceServiceRequest>,
    ) -> Result<Response<ExportTraceServiceResponse>, Status> {
        self.forward(OtlpRequest::Traces(request.into_inner()))
            .await?;
        Ok(Response::new(ExportTraceServiceResponse {
            partial_success: None,
        }))
    }
}

#[tonic::async_trait]
impl ProfilesService for OtlpReceiver {
    async fn export(
        &self,
        request: Request<ExportProfilesServiceRequest>,
    ) -> Result<Response<ExportProfilesServiceResponse>, Status> {
        self.forward(OtlpRequest::Profiles(request.into_inner()))
            .await?;
        Ok(Response::new(ExportProfilesServiceResponse {
            partial_success: None,
        }))
    }
}

/// The largest OTLP/HTTP request body accepted.
const MAX_HTTP_BODY_BYTES: usize = 20 * 1024 * 1024;

/// The content type of a binary protobuf OTLP/HTTP body.
const PROTOBUF: &str = "application/x-protobuf";

/// An export request that has an OTLP/HTTP endpoint.
trait HttpExport: Message + Default + Into<OtlpRequest> {
    /// The message that acknowledges the export.
    type Response: Message + Default;
}

impl HttpExport for ExportLogsServiceRequest {
    type Response = ExportLogsServiceResponse;
}

impl HttpExport for ExportMetricsServiceRequest {
    type Response = ExportMetricsServiceResponse;
}

impl HttpExport for ExportTraceServiceRequest {
    type Response = ExportTraceServiceResponse;
}

impl HttpExport for ExportProfilesServiceRequest {
    type Response = ExportProfilesServiceResponse;
}

/// `google.rpc.Status`, the body of an OTLP/HTTP error response.
#[derive(Clone, PartialEq, Message)]
struct HttpStatus {
    #[prost(int32, tag = "1")]
    code: i32,
    #[prost(string, tag = "2")]
    message: String,
}

fn protobuf_response(status: StatusCode, message: &impl Message) -> axum::response::Response {
    (status, [(CONTENT_TYPE, PROTOBUF)], message.encode_to_vec()).into_response()
}

fn http_error(status: StatusCode, code: Code, message: &str) -> axum::response::Response {
    protobuf_response(
        status,
        &HttpStatus {
            code: code as i32,
            message: message.to_owned(),
        },
    )
}

/// Whether the body is uncompressed binary protobuf.
fn is_plain_protobuf(headers: &HeaderMap) -> bool {
    let header = |name| headers.get(name).and_then(|value| value.to_str().ok());
    let media_type = header(CONTENT_TYPE)
        .and_then(|value| value.split(';').next())
        .map(str::trim);
    let encoding = header(CONTENT_ENCODING).unwrap_or("identity");
    media_type.is_some_and(|media_type| media_type.eq_ignore_ascii_case(PROTOBUF))
        && encoding.eq_ignore_ascii_case("identity")
}

/// Handles one OTLP/HTTP export.
async fn export_http<T: HttpExport>(
    State(receiver): State<OtlpReceiver>,
    headers: HeaderMap,
    body: Bytes,
) -> axum::response::Response {
    if !is_plain_protobuf(&headers) {
        return http_error(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            Code::InvalidArgument,
            "only uncompressed application/x-protobuf bodies are supported",
        );
    }
    let Ok(request) = T::decode(body) else {
        return http_error(
            StatusCode::BAD_REQUEST,
            Code::InvalidArgument,
            "the body is not a valid OTLP export request",
        );
    };
    match receiver.forward(request.into()).await {
        Ok(()) => protobuf_response(StatusCode::OK, &T::Response::default()),
        Err(status) => http_error(
            StatusCode::SERVICE_UNAVAILABLE,
            status.code(),
            status.message(),
        ),
    }
}
