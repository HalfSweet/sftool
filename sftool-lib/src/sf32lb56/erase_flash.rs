use super::SF32LB56Tool;
use crate::common::erase_flash::EraseOps;
use crate::erase_flash::EraseFlashTrait;
use crate::{EraseFlashParams, EraseRegionParams, ProgressCallback};
use std::future::Future;
use std::pin::Pin;

impl EraseFlashTrait for SF32LB56Tool {
    fn erase_flash<'a>(
        &'a mut self, 
        params: &'a EraseFlashParams,
        progress_callback: Option<ProgressCallback>
    ) -> Pin<Box<dyn Future<Output = Result<(), std::io::Error>> + Send + 'a>> {
        Box::pin(async move {
            if let Some(ref callback) = progress_callback {
                callback(crate::ProgressInfo {
                    step: 1,
                    total_steps: Some(1),
                    current_file: None,
                    bytes_processed: 0,
                    total_bytes: None,
                    message: format!("Erasing entire flash at 0x{:08X}", params.address),
                });
            }
            
            let result = EraseOps::erase_all(self, params.address);
            
            if let Some(ref callback) = progress_callback {
                callback(crate::ProgressInfo {
                    step: 2,
                    total_steps: Some(1),
                    current_file: None,
                    bytes_processed: 0,
                    total_bytes: None,
                    message: "Flash erase completed".to_string(),
                });
            }
            
            result
        })
    }

    fn erase_region<'a>(
        &'a mut self, 
        params: &'a EraseRegionParams,
        progress_callback: Option<ProgressCallback>
    ) -> Pin<Box<dyn Future<Output = Result<(), std::io::Error>> + Send + 'a>> {
        Box::pin(async move {
            let mut current_step = 1u32;
            
            // 处理每个区域
            for (i, region) in params.regions.iter().enumerate() {
                if let Some(ref callback) = progress_callback {
                    callback(crate::ProgressInfo {
                        step: current_step,
                        total_steps: Some(params.regions.len() as u32),
                        current_file: None,
                        bytes_processed: 0,
                        total_bytes: None,
                        message: format!("Erasing region {}/{} at 0x{:08X}", i + 1, params.regions.len(), region.address),
                    });
                }
                
                EraseOps::erase_region(self, region.address, region.size)?;
                current_step += 1;
            }
            Ok(())
        })
    }
}
