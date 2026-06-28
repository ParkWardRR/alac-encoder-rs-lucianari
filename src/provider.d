provider alac {
    probe encode__frame__start(uint32_t frame_size, uint32_t channels);
    probe encode__frame__end(uint32_t bytes_written);
    probe buffer__flush(uint32_t bytes_flushed);
};
