package alac

import (
	"bytes"
	"testing"
)

func TestEncoderAndWriter(t *testing.T) {
	cfg := Config{
		FrameSize:  352,
		Channels:   2,
		Layout:     1,
		BitDepth:   16,
		SampleRate: 44100,
	}

	var buf bytes.Buffer
	w, err := NewWriter(&buf, cfg)
	if err != nil {
		t.Fatalf("Failed to create writer: %v", err)
	}

	// Create some dummy PCM (silence)
	pcm := make([]byte, 352*2*2*5) // 5 frames
	
	n, err := w.Write(pcm)
	if err != nil {
		t.Fatalf("Write failed: %v", err)
	}
	if n != len(pcm) {
		t.Fatalf("Short write: %d", n)
	}

	err = w.Close()
	if err != nil {
		t.Fatalf("Close failed: %v", err)
	}

	if buf.Len() == 0 {
		t.Fatal("Expected compressed ALAC bytes, got 0")
	}

	// Silence should compress extremely well
	if buf.Len() > 2000 {
		t.Fatalf("Silence compressed to %d bytes, expected less overhead", buf.Len())
	}
}
