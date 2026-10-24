use crate::records::*;
use chipdock_service::api::ChipDockCore as Service;
use std::sync::Arc;

/// One native client handle; the shared service owns progress and exclusion.
#[derive(uniffi::Object)]
pub struct ChipDockCore {
    service: Arc<Service>,
}

#[uniffi::export]
impl ChipDockCore {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            service: Service::new(),
        })
    }

    pub fn version(&self) -> String {
        self.service.version()
    }

    pub fn list_probes(&self) -> Result<Vec<ProbeSummary>, CoreError> {
        self.service.list_probes()
    }

    pub fn connect_target(&self, request: ConnectRequest) -> Result<ConnectionInfo, CoreError> {
        self.service.connect_target(request)
    }

    pub fn search_chips(&self, query: String, limit: u32) -> Result<Vec<String>, CoreError> {
        self.service.search_chips(query, limit)
    }

    pub fn target_memory_map(&self, chip: String) -> Result<Vec<MemoryRegionLayout>, CoreError> {
        self.service.target_memory_map(chip)
    }

    pub fn firmware_usage(&self, input: FirmwareInput) -> Result<FirmwareUsage, CoreError> {
        self.service.firmware_usage(input)
    }

    pub fn firmware_preview(
        &self,
        input: FirmwareInput,
        length: u32,
    ) -> Result<FirmwarePreview, CoreError> {
        self.service.firmware_preview(input, length)
    }

    pub fn flash_firmware(&self, request: FlashRequest) -> Result<String, CoreError> {
        self.service.flash_firmware(request)
    }

    pub fn erase_target(&self, request: EraseRequest) -> Result<String, CoreError> {
        self.service.erase_target(request)
    }

    pub fn read_memory(&self, request: MemoryRequest) -> Result<MemoryReadResult, CoreError> {
        self.service.read_memory(request)
    }

    pub fn take_events(&self) -> Vec<ProgressEvent> {
        self.service.take_events()
    }

    pub fn load_profiles(&self, directory: String) -> Result<Vec<SavedProfile>, CoreError> {
        self.service.load_profiles(directory)
    }

    pub fn save_profiles(
        &self,
        directory: String,
        profiles: Vec<SavedProfile>,
    ) -> Result<(), CoreError> {
        self.service.save_profiles(directory, profiles)
    }

    pub fn load_recent_files(&self, directory: String) -> Result<Vec<RecentFirmware>, CoreError> {
        self.service.load_recent_files(directory)
    }

    pub fn save_recent_files(
        &self,
        directory: String,
        files: Vec<RecentFirmware>,
    ) -> Result<(), CoreError> {
        self.service.save_recent_files(directory, files)
    }
}
