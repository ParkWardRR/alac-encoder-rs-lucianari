#ifndef ALAC_H
#define ALAC_H

#include <stddef.h>
#include <stdint.h>

typedef struct {
    unsigned int frame_size;
    unsigned int channels;
    unsigned int layout;
    unsigned int bit_depth;
    unsigned int sample_rate;
} CAlacConfig;

void* alac_encoder_create(const CAlacConfig* config);
void alac_encoder_free(void* encoder_ptr);
size_t alac_encoder_required_workspace(unsigned int channels, unsigned int frame_size);

ptrdiff_t alac_encoder_encode(
    void* encoder_ptr,
    const unsigned char* pcm_data,
    size_t pcm_len,
    int* workspace,
    size_t workspace_len,
    unsigned char* out_buffer,
    size_t out_len
);

#endif
