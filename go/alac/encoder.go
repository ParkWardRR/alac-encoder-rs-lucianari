package alac

/*
#cgo LDFLAGS: -L../../target/release -lalac_encoder_rs_lucianari -lm
#if defined(__APPLE__)
#cgo LDFLAGS: -framework CoreFoundation
#endif
#include "alac.h"
#include <stdlib.h>
*/
import "C"
import (
	"errors"
	"runtime"
	"unsafe"
)

type Config struct {
	FrameSize  uint32
	Channels   uint32
	Layout     uint32
	BitDepth   uint32
	SampleRate uint32
}

type Encoder struct {
	ptr       unsafe.Pointer
	workspace []int32
	config    Config
}

func NewEncoder(config Config) (*Encoder, error) {
	cConfig := C.CAlacConfig{
		frame_size:  C.uint(config.FrameSize),
		channels:    C.uint(config.Channels),
		layout:      C.uint(config.Layout),
		bit_depth:   C.uint(config.BitDepth),
		sample_rate: C.uint(config.SampleRate),
	}

	ptr := C.alac_encoder_create(&cConfig)
	if ptr == nil {
		return nil, errors.New("failed to create ALAC encoder")
	}

	wsSize := C.alac_encoder_required_workspace(cConfig.channels, cConfig.frame_size)
	workspace := make([]int32, int(wsSize))

	enc := &Encoder{
		ptr:       ptr,
		workspace: workspace,
		config:    config,
	}
	
	// Ensure the Rust side is freed when Go garbage collects the encoder
	runtime.SetFinalizer(enc, func(e *Encoder) {
		e.Close()
	})

	return enc, nil
}

func (e *Encoder) Close() {
	if e.ptr != nil {
		C.alac_encoder_free(e.ptr)
		e.ptr = nil
	}
}

// Encode encodes a raw PCM buffer into the provided output ALAC buffer.
// Returns the number of bytes written to out.
func (e *Encoder) Encode(pcm []byte, out []byte) (int, error) {
	if e.ptr == nil {
		return 0, errors.New("encoder is closed")
	}
	if len(pcm) == 0 {
		return 0, nil
	}

	var pcmPtr *C.uchar
	if len(pcm) > 0 {
		pcmPtr = (*C.uchar)(unsafe.Pointer(&pcm[0]))
	}

	var wsPtr *C.int
	if len(e.workspace) > 0 {
		wsPtr = (*C.int)(unsafe.Pointer(&e.workspace[0]))
	}

	var outPtr *C.uchar
	if len(out) > 0 {
		outPtr = (*C.uchar)(unsafe.Pointer(&out[0]))
	}

	res := C.alac_encoder_encode(
		e.ptr,
		pcmPtr,
		C.size_t(len(pcm)),
		wsPtr,
		C.size_t(len(e.workspace)),
		outPtr,
		C.size_t(len(out)),
	)

	if res < 0 {
		return 0, errors.New("alac encoding failed")
	}

	return int(res), nil
}
