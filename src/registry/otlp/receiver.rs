// SPDX-License-Identifier: Apache-2.0

//! The OTLP/gRPC services, as an axum router that queues exports for the checker.

use std::sync::Arc;

use axum::Router;
use tonic::service::Routes;
use tonic::{Request, Response, Status};

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
