#[cfg(feature = "grpc")]
pub mod proto {
    tonic::include_proto!("alac");
}

#[cfg(feature = "grpc")]
pub mod server {
    use super::proto::alac_encoder_server::AlacEncoder;
    use super::proto::{EncodeRequest, EncodeResponse};
    use crate::encoder::{AlacConfig, AlacEncoder as CoreEncoder, ChannelLayout};
    use tokio::sync::mpsc;
    use tokio_stream::wrappers::ReceiverStream;
    use tonic::{Request, Response, Status, Streaming};

    #[derive(Default)]
    pub struct AlacGrpcService {}

    #[tonic::async_trait]
    impl AlacEncoder for AlacGrpcService {
        type EncodeStreamStream = ReceiverStream<Result<EncodeResponse, Status>>;

        async fn encode_stream(
            &self,
            request: Request<Streaming<EncodeRequest>>,
        ) -> Result<Response<Self::EncodeStreamStream>, Status> {
            let mut stream = request.into_inner();

            let (tx, rx) = mpsc::channel(128);

            tokio::spawn(async move {
                let mut encoder: Option<CoreEncoder> = None;
                let mut workspace: Vec<i32> = Vec::new();
                let mut out_buffer: Vec<u8> = vec![0; 16384]; // Large enough bound

                while let Some(msg) = stream.message().await.unwrap_or(None) {
                    match msg.payload {
                        Some(super::proto::encode_request::Payload::Config(cfg)) => {
                            let layout = match cfg.layout {
                                0 => ChannelLayout::Mono,
                                1 => ChannelLayout::Stereo,
                                2 => ChannelLayout::Surround5Point1,
                                3 => ChannelLayout::Surround7Point1,
                                _ => ChannelLayout::Custom(cfg.channels),
                            };
                            let config = AlacConfig {
                                frame_size: cfg.frame_size,
                                channels: cfg.channels,
                                layout,
                                bit_depth: cfg.bit_depth,
                                sample_rate: cfg.sample_rate,
                            };
                            encoder = Some(CoreEncoder::new(config.clone()));
                            workspace = vec![0; CoreEncoder::required_workspace(cfg.channels, cfg.frame_size)];
                        }
                        Some(super::proto::encode_request::Payload::PcmChunk(pcm)) => {
                            if let Some(ref mut enc) = encoder {
                                if pcm.is_empty() {
                                    continue;
                                }
                                // In a real implementation we would buffer to exact frame size.
                                // For brevity, assuming the client sends exact frame size chunks.
                                match enc.encode(&pcm, &mut workspace, &mut out_buffer) {
                                    Ok(size) => {
                                        let resp = EncodeResponse {
                                            alac_chunk: out_buffer[..size].to_vec(),
                                        };
                                        if tx.send(Ok(resp)).await.is_err() {
                                            break;
                                        }
                                    }
                                    Err(_) => {
                                        let _ = tx.send(Err(Status::internal("Encoding failed"))).await;
                                        break;
                                    }
                                }
                            } else {
                                let _ = tx.send(Err(Status::failed_precondition("Config not sent first"))).await;
                                break;
                            }
                        }
                        None => {}
                    }
                }
            });

            Ok(Response::new(ReceiverStream::new(rx)))
        }
    }
}
