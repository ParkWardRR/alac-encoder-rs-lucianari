/// Defines extended immersive channel beds beyond standard 7.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImmersiveChannelLayout {
    Atmos714, // 7.1 + 4 height channels
    Atmos916, // 9.1 + 6 height channels
    Nhk22_2,  // 22.2 surround
}

pub struct ImmersiveLayoutMatrix {
    pub layout: ImmersiveChannelLayout,
}

impl ImmersiveLayoutMatrix {
    pub fn new(layout: ImmersiveChannelLayout) -> Self {
        Self { layout }
    }

    /// Returns the total number of discrete channels required.
    pub fn total_channels(&self) -> usize {
        match self.layout {
            ImmersiveChannelLayout::Atmos714 => 12,
            ImmersiveChannelLayout::Atmos916 => 16,
            ImmersiveChannelLayout::Nhk22_2 => 24,
        }
    }
}
