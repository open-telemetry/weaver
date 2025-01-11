// This file is @generated by prost-build.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ExportLogsServiceRequest {
    /// An array of ResourceLogs.
    /// For data coming from a single resource this array will typically contain one
    /// element. Intermediary nodes (such as OpenTelemetry Collector) that receive
    /// data from multiple origins typically batch the data before forwarding further and
    /// in that case this array will contain multiple elements.
    #[prost(message, repeated, tag = "1")]
    pub resource_logs: ::prost::alloc::vec::Vec<
        super::super::super::logs::v1::ResourceLogs,
    >,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ExportLogsServiceResponse {
    /// The details of a partially successful export request.
    ///
    /// If the request is only partially accepted
    /// (i.e. when the server accepts only parts of the data and rejects the rest)
    /// the server MUST initialize the `partial_success` field and MUST
    /// set the `rejected_<signal>` with the number of items it rejected.
    ///
    /// Servers MAY also make use of the `partial_success` field to convey
    /// warnings/suggestions to senders even when the request was fully accepted.
    /// In such cases, the `rejected_<signal>` MUST have a value of `0` and
    /// the `error_message` MUST be non-empty.
    ///
    /// A `partial_success` message with an empty value (rejected\_<signal> = 0 and
    /// `error_message` = "") is equivalent to it not being set/present. Senders
    /// SHOULD interpret it the same way as in the full success case.
    #[prost(message, optional, tag = "1")]
    pub partial_success: ::core::option::Option<ExportLogsPartialSuccess>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ExportLogsPartialSuccess {
    /// The number of rejected log records.
    ///
    /// A `rejected_<signal>` field holding a `0` value indicates that the
    /// request was fully accepted.
    #[prost(int64, tag = "1")]
    pub rejected_log_records: i64,
    /// A developer-facing human-readable message in English. It should be used
    /// either to explain why the server rejected parts of the data during a partial
    /// success or to convey warnings/suggestions during a full success. The message
    /// should offer guidance on how users can address such issues.
    ///
    /// error_message is an optional field. An error_message with an empty value
    /// is equivalent to it not being set.
    #[prost(string, tag = "2")]
    pub error_message: ::prost::alloc::string::String,
}
/// Generated client implementations.
pub mod logs_service_client {
    #![allow(
        unused_variables,
        dead_code,
        missing_docs,
        clippy::wildcard_imports,
        clippy::let_unit_value,
    )]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    /// Service that can be used to push logs between one Application instrumented with
    /// OpenTelemetry and an collector, or between an collector and a central collector (in this
    /// case logs are sent/received to/from multiple Applications).
    #[derive(Debug, Clone)]
    pub struct LogsServiceClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl LogsServiceClient<tonic::transport::Channel> {
        /// Attempt to create a new client by connecting to a given endpoint.
        pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
        where
            D: TryInto<tonic::transport::Endpoint>,
            D::Error: Into<StdError>,
        {
            let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
            Ok(Self::new(conn))
        }
    }
    impl<T> LogsServiceClient<T>
    where
        T: tonic::client::GrpcService<tonic::body::BoxBody>,
        T::Error: Into<StdError>,
        T::ResponseBody: Body<Data = Bytes> + std::marker::Send + 'static,
        <T::ResponseBody as Body>::Error: Into<StdError> + std::marker::Send,
    {
        pub fn new(inner: T) -> Self {
            let inner = tonic::client::Grpc::new(inner);
            Self { inner }
        }
        pub fn with_origin(inner: T, origin: Uri) -> Self {
            let inner = tonic::client::Grpc::with_origin(inner, origin);
            Self { inner }
        }
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> LogsServiceClient<InterceptedService<T, F>>
        where
            F: tonic::service::Interceptor,
            T::ResponseBody: Default,
            T: tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
                Response = http::Response<
                    <T as tonic::client::GrpcService<tonic::body::BoxBody>>::ResponseBody,
                >,
            >,
            <T as tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
            >>::Error: Into<StdError> + std::marker::Send + std::marker::Sync,
        {
            LogsServiceClient::new(InterceptedService::new(inner, interceptor))
        }
        /// Compress requests with the given encoding.
        ///
        /// This requires the server to support it otherwise it might respond with an
        /// error.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.send_compressed(encoding);
            self
        }
        /// Enable decompressing responses.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.accept_compressed(encoding);
            self
        }
        /// Limits the maximum size of a decoded message.
        ///
        /// Default: `4MB`
        #[must_use]
        pub fn max_decoding_message_size(mut self, limit: usize) -> Self {
            self.inner = self.inner.max_decoding_message_size(limit);
            self
        }
        /// Limits the maximum size of an encoded message.
        ///
        /// Default: `usize::MAX`
        #[must_use]
        pub fn max_encoding_message_size(mut self, limit: usize) -> Self {
            self.inner = self.inner.max_encoding_message_size(limit);
            self
        }
        /// For performance reasons, it is recommended to keep this RPC
        /// alive for the entire life of the application.
        pub async fn export(
            &mut self,
            request: impl tonic::IntoRequest<super::ExportLogsServiceRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ExportLogsServiceResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::unknown(
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/opentelemetry.proto.collector.logs.v1.LogsService/Export",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new(
                        "opentelemetry.proto.collector.logs.v1.LogsService",
                        "Export",
                    ),
                );
            self.inner.unary(req, path, codec).await
        }
    }
}
