package alac

import (
	"io"
)

// Writer wraps an underlying io.Writer, compressing any incoming raw PCM bytes
// into ALAC frames and writing them downstream.
type Writer struct {
	w             io.Writer
	enc           *Encoder
	pcmBuffer     []byte
	outBuffer     []byte
	bytesPerFrame int
}

// NewWriter creates a new ALAC compressing io.Writer.
func NewWriter(w io.Writer, config Config) (*Writer, error) {
	enc, err := NewEncoder(config)
	if err != nil {
		return nil, err
	}

	bytesPerFrame := int(config.FrameSize * config.Channels * (config.BitDepth / 8))

	return &Writer{
		w:             w,
		enc:           enc,
		pcmBuffer:     make([]byte, 0, bytesPerFrame),
		outBuffer:     make([]byte, bytesPerFrame*2), // safe bound
		bytesPerFrame: bytesPerFrame,
	}, nil
}

// Write consumes raw PCM audio and flushes ALAC frames to the underlying writer.
func (aw *Writer) Write(p []byte) (n int, err error) {
	written := 0
	for len(p) > 0 {
		spaceLeft := aw.bytesPerFrame - len(aw.pcmBuffer)
		if spaceLeft == 0 {
			if err := aw.flushFrame(); err != nil {
				return written, err
			}
			spaceLeft = aw.bytesPerFrame
		}

		toCopy := len(p)
		if toCopy > spaceLeft {
			toCopy = spaceLeft
		}

		aw.pcmBuffer = append(aw.pcmBuffer, p[:toCopy]...)
		p = p[toCopy:]
		written += toCopy
	}
	return written, nil
}

// Close flushes any remaining partial frames and frees the encoder.
func (aw *Writer) Close() error {
	if len(aw.pcmBuffer) > 0 {
		if err := aw.flushFrame(); err != nil {
			return err
		}
	}
	aw.enc.Close()
	return nil
}

func (aw *Writer) flushFrame() error {
	if len(aw.pcmBuffer) == 0 {
		return nil
	}

	n, err := aw.enc.Encode(aw.pcmBuffer, aw.outBuffer)
	if err != nil {
		return err
	}

	_, err = aw.w.Write(aw.outBuffer[:n])
	if err != nil {
		return err
	}

	aw.pcmBuffer = aw.pcmBuffer[:0]
	return nil
}
