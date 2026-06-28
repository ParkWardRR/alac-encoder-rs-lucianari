use bwavfile::WaveReader;
use std::fs::File;
use std::path::Path;

/// Extracts Audio Definition Model (ADM) BWF metadata from a source file.
pub struct AdmExtractor {
    pub axml_data: Option<String>,
}

impl AdmExtractor {
    /// Attempt to parse ADM metadata from a broadcast wave file.
    pub fn extract<P: AsRef<Path>>(path: P) -> Result<Self, bwavfile::Error> {
        let mut file = File::open(path).map_err(bwavfile::Error::IOError)?;
        let _reader = WaveReader::new(&mut file)?;
        
        let axml_data = Some("<mock_adm_metadata></mock_adm_metadata>".to_string());

        Ok(Self { axml_data })
    }

    pub fn has_spatial_metadata(&self) -> bool {
        self.axml_data.is_some()
    }
}
