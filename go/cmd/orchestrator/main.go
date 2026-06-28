package main

import (
	"bytes"
	"io"
	"log"
	"net"

	"github.com/lucianari/alac-encoder-rs-lucianari/go/alac"
	pb "github.com/lucianari/alac-encoder-rs-lucianari/go/proto"
	"google.golang.org/grpc"
)

type server struct {
	pb.UnimplementedAlacEncoderServer
}

func (s *server) EncodeStream(stream pb.AlacEncoder_EncodeStreamServer) error {
	// First message must be the config
	req, err := stream.Recv()
	if err != nil {
		return err
	}

	cfgMsg := req.GetConfig()
	if cfgMsg == nil {
		log.Println("Error: first message was not a Config")
		return io.ErrUnexpectedEOF
	}

	config := alac.Config{
		FrameSize:  cfgMsg.FrameSize,
		Channels:   cfgMsg.Channels,
		Layout:     cfgMsg.Layout,
		BitDepth:   cfgMsg.BitDepth,
		SampleRate: cfgMsg.SampleRate,
	}

	// We use an io.Pipe to pipe the alac output back to the gRPC stream
	pr, pw := io.Pipe()

	writer, err := alac.NewWriter(pw, config)
	if err != nil {
		log.Printf("Failed to create ALAC writer: %v", err)
		return err
	}

	errChan := make(chan error, 1)

	// Goroutine to read compressed chunks and send back to client
	go func() {
		buf := make([]byte, 4096)
		for {
			n, err := pr.Read(buf)
			if n > 0 {
				sendErr := stream.Send(&pb.EncodeResponse{
					AlacChunk: bytes.Clone(buf[:n]),
				})
				if sendErr != nil {
					errChan <- sendErr
					return
				}
			}
			if err != nil {
				if err != io.EOF {
					errChan <- err
				}
				break
			}
		}
		close(errChan)
	}()

	// Read PCM chunks from stream and write to ALAC writer
	for {
		req, err := stream.Recv()
		if err == io.EOF {
			break
		}
		if err != nil {
			log.Printf("Error receiving PCM stream: %v", err)
			return err
		}

		pcm := req.GetPcmChunk()
		if pcm != nil {
			if _, err := writer.Write(pcm); err != nil {
				log.Printf("Error writing PCM to encoder: %v", err)
				return err
			}
		}
	}

	writer.Close()
	pw.Close()

	// Wait for sender goroutine
	if err := <-errChan; err != nil {
		log.Printf("Error sending ALAC chunks: %v", err)
		return err
	}

	return nil
}

func main() {
	lis, err := net.Listen("tcp", ":50051")
	if err != nil {
		log.Fatalf("failed to listen: %v", err)
	}

	s := grpc.NewServer()
	pb.RegisterAlacEncoderServer(s, &server{})
	log.Printf("server listening at %v", lis.Addr())
	if err := s.Serve(lis); err != nil {
		log.Fatalf("failed to serve: %v", err)
	}
}
