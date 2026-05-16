//! Fallback platform backend for targets that have no implementation.
//! Every operation returns [`CoreError::UnsupportedPlatform`].

use crate::{CoreError, DiskInfo, InstallRequest, PartitionInfo, ProgressSink, Result};

pub fn list_disks() -> Result<Vec<DiskInfo>> {
    Err(CoreError::UnsupportedPlatform)
}

pub fn list_partitions(_device: String) -> Result<Vec<PartitionInfo>> {
    Err(CoreError::UnsupportedPlatform)
}

pub fn install(_req: InstallRequest, _sink: &dyn ProgressSink) -> Result<()> {
    Err(CoreError::UnsupportedPlatform)
}
