//! Streaming ALAC Encoder
//!
//! Provides a high-level `std::io::Write` wrapper that chunks raw PCM audio
//! into optimal ALAC frames and encodes them sequentially.

use crate::encoder::{AlacConfig, AlacEncoder};
use alloc::string::ToString;
use alloc::vec::Vec;
use alloc::vec;

use std::io::{self, Write};

/// An ALAC stream writer that buffers raw PCM data and flushes compressed frames
/// to an underlying `Write` sink.
pub struct AlacStreamWriter<W: Write> {
    writer: W,
    encoder: AlacEncoder,
    pcm_buffer: Vec<u8>,
    out_buffer: Vec<u8>,
    workspace: Vec<i32>,
    bytes_per_frame: usize,
}

impl<W: Write> AlacStreamWriter<W> {
    /// Create a new stream writer.
    pub fn new(writer: W, config: AlacConfig) -> Self {
        let bytes_per_sample = (config.bit_depth / 8) as usize;
        let bytes_per_frame = (config.frame_size as usize) * (config.channels as usize) * bytes_per_sample;
        
        // Worst-case buffer sizes
        let out_size = bytes_per_frame + 16384; 
        
        Self {
            writer,
            encoder: AlacEncoder::new(config.clone()),
            pcm_buffer: Vec::with_capacity(bytes_per_frame),
            out_buffer: vec![0; out_size],
            workspace: vec![0; AlacEncoder::required_workspace(config.channels, config.frame_size)],
            bytes_per_frame,
        }
    }
    
    /// Write raw PCM data. It will be buffered and encoded into ALAC frames
    /// as enough data becomes available.
    pub fn write_pcm(&mut self, data: &[u8]) -> io::Result<()> {
        let mut offset = 0;
        
        while offset < data.len() {
            let space_left = self.bytes_per_frame - self.pcm_buffer.len();
            let chunk_size = std::cmp::min(space_left, data.len() - offset);
            
            self.pcm_buffer.extend_from_slice(&data[offset..offset + chunk_size]);
            offset += chunk_size;
            
            if self.pcm_buffer.len() == self.bytes_per_frame {
                self.flush_frame()?;
            }
        }
        Ok(())
    }
    
    /// Flush any remaining PCM data as a partial frame.
    pub fn flush_frame(&mut self) -> io::Result<()> {
        if self.pcm_buffer.is_empty() {
            return Ok(());
        }
        
        let encoded_bytes = self.encoder
            .encode(&self.pcm_buffer, &mut self.workspace, &mut self.out_buffer)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
            
        self.writer.write_all(&self.out_buffer[..encoded_bytes])?;
        self.pcm_buffer.clear();
        Ok(())
    }
    
    /// Flush any remaining data and return the underlying writer.
    pub fn into_inner(mut self) -> io::Result<W> {
        self.flush_frame()?;
        Ok(self.writer)
    }
}

impl<W: Write> Write for AlacStreamWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.write_pcm(buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flush_frame()?;
        self.writer.flush()
    }
}

#[cfg(feature = "async")]
pub mod async_stream {
    use crate::encoder::{AlacConfig, AlacEncoder};
    use alloc::string::ToString;
    use alloc::vec::Vec;
    use alloc::vec;
    use std::io;
    use std::pin::Pin;
    use std::task::{Context, Poll};
    use tokio::io::AsyncWrite;

    enum State {
        AcceptingPcm,
        WritingFrame { total: usize, written: usize },
    }

    pub struct AlacAsyncStreamWriter<W: AsyncWrite + Unpin> {
        writer: W,
        encoder: AlacEncoder,
        pcm_buffer: Vec<u8>,
        out_buffer: Vec<u8>,
        workspace: Vec<i32>,
        bytes_per_frame: usize,
        state: State,
    }

    impl<W: AsyncWrite + Unpin> AlacAsyncStreamWriter<W> {
        pub fn new(writer: W, config: AlacConfig) -> Self {
            let bytes_per_sample = (config.bit_depth / 8) as usize;
            let bytes_per_frame = (config.frame_size as usize) * (config.channels as usize) * bytes_per_sample;
            let out_size = bytes_per_frame + 16384; 
            
            Self {
                writer,
                encoder: AlacEncoder::new(config.clone()),
                pcm_buffer: Vec::with_capacity(bytes_per_frame),
                out_buffer: vec![0; out_size],
                workspace: vec![0; AlacEncoder::required_workspace(config.channels, config.frame_size)],
                bytes_per_frame,
                state: State::AcceptingPcm,
            }
        }
        
        pub fn into_inner(self) -> W {
            self.writer
        }
    }

    impl<W: AsyncWrite + Unpin> AsyncWrite for AlacAsyncStreamWriter<W> {
        fn poll_write(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            let mut this = self.as_mut();
            
            loop {
                match this.state {
                    State::AcceptingPcm => {
                        let space_left = this.bytes_per_frame - this.pcm_buffer.len();
                        if space_left == 0 {
                            // Buffer is full, need to encode and switch state
                            let AlacAsyncStreamWriter { encoder, pcm_buffer, workspace, out_buffer, .. } = &mut *this;
                            let encoded_bytes = encoder
                                .encode(pcm_buffer, workspace, out_buffer)
                                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
                            
                            this.pcm_buffer.clear();
                            this.state = State::WritingFrame { total: encoded_bytes, written: 0 };
                            continue;
                        }
                        
                        let chunk_size = std::cmp::min(space_left, buf.len());
                        if chunk_size == 0 {
                            return Poll::Ready(Ok(0));
                        }
                        
                        this.pcm_buffer.extend_from_slice(&buf[..chunk_size]);
                        return Poll::Ready(Ok(chunk_size));
                    }
                    State::WritingFrame { total, mut written } => {
                        while written < total {
                            let AlacAsyncStreamWriter { writer, out_buffer, .. } = &mut *this;
                            match Pin::new(writer).poll_write(cx, &out_buffer[written..total]) {
                                Poll::Ready(Ok(n)) => {
                                    if n == 0 {
                                        return Poll::Ready(Err(io::Error::new(io::ErrorKind::WriteZero, "failed to write whole frame")));
                                    }
                                    written += n;
                                    this.state = State::WritingFrame { total, written };
                                }
                                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                                Poll::Pending => return Poll::Pending,
                            }
                        }
                        // Frame fully written, go back to accepting
                        this.state = State::AcceptingPcm;
                    }
                }
            }
        }

        fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            let mut this = self.as_mut();
            loop {
                match this.state {
                    State::AcceptingPcm => {
                        if this.pcm_buffer.is_empty() {
                            return Pin::new(&mut this.writer).poll_flush(cx);
                        }
                        // Need to encode the partial buffer
                        let AlacAsyncStreamWriter { encoder, pcm_buffer, workspace, out_buffer, .. } = &mut *this;
                        let encoded_bytes = encoder
                            .encode(pcm_buffer, workspace, out_buffer)
                            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
                        
                        this.pcm_buffer.clear();
                        this.state = State::WritingFrame { total: encoded_bytes, written: 0 };
                    }
                    State::WritingFrame { total, mut written } => {
                        while written < total {
                            let AlacAsyncStreamWriter { writer, out_buffer, .. } = &mut *this;
                            match Pin::new(writer).poll_write(cx, &out_buffer[written..total]) {
                                Poll::Ready(Ok(n)) => {
                                    if n == 0 {
                                        return Poll::Ready(Err(io::Error::new(io::ErrorKind::WriteZero, "failed to write whole frame")));
                                    }
                                    written += n;
                                    this.state = State::WritingFrame { total, written };
                                }
                                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                                Poll::Pending => return Poll::Pending,
                            }
                        }
                        this.state = State::AcceptingPcm;
                    }
                }
            }
        }

        fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            // First flush any remaining data
            match self.as_mut().poll_flush(cx) {
                Poll::Ready(Ok(())) => Pin::new(&mut self.writer).poll_shutdown(cx),
                other => other,
            }
        }
    }
}

#[cfg(all(feature = "io_uring", target_os = "linux"))]
pub struct AlacUringWriter<W> {
    inner: W,
    encoder: crate::encoder::AlacEncoder,
    workspace: Vec<i32>,
    out_buffer: Vec<u8>,
}

#[cfg(all(feature = "io_uring", target_os = "linux"))]
impl<W: std::os::unix::io::AsRawFd> AlacUringWriter<W> {
    pub fn new(writer: W, config: crate::encoder::AlacConfig) -> Self {
        let ws_size = crate::encoder::AlacEncoder::required_workspace(config.channels as usize, config.frame_size as usize);
        Self {
            inner: writer,
            encoder: crate::encoder::AlacEncoder::new(config),
            workspace: vec![0; ws_size],
            out_buffer: vec![0; 16384],
        }
    }

    pub async fn encode_and_write(&mut self, pcm: &[u8]) -> std::io::Result<usize> {
        match self.encoder.encode(pcm, &mut self.workspace, &mut self.out_buffer) {
            Ok(size) => {
                // In a real io_uring implementation, we'd use tokio_uring::fs::File or similar.
                // This is an architecture placeholder.
                Ok(size)
            }
            Err(e) => Err(std::io::Error::new(std::io::ErrorKind::Other, "encoding failed")),
        }
    }
}
